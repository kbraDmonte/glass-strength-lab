//! A minimal .xlsx writer for the data table: one sheet per entry, bold and
//! frozen header row with Excel's filter buttons, column widths from the
//! content. Cells that are numbers (or text that reads as a plain number)
//! become numbers in Excel; "–" (empty) becomes an empty cell.

use serde_json::Value;

/// A zip archive without compression ("stored"): local headers, then the
/// central directory. Small to write by hand, and every xlsx reader accepts it.
#[derive(Default)]
struct Zip {
    out: Vec<u8>,
    dir: Vec<u8>,
    count: u16,
}

impl Zip {
    fn put(&mut self, name: &str, data: &[u8]) {
        let crc = crc32fast::hash(data);
        let (len, offset) = (data.len() as u32, self.out.len() as u32);
        let name = name.as_bytes();
        // version 2.0, flag bit 11 = UTF-8 names, method 0 = stored, time/date 1980-01-01
        let common = |v: &mut Vec<u8>| {
            v.extend_from_slice(&20u16.to_le_bytes());
            v.extend_from_slice(&0x0800u16.to_le_bytes());
            v.extend_from_slice(&0u16.to_le_bytes());
            v.extend_from_slice(&0u16.to_le_bytes());
            v.extend_from_slice(&0x0021u16.to_le_bytes());
            v.extend_from_slice(&crc.to_le_bytes());
            v.extend_from_slice(&len.to_le_bytes());
            v.extend_from_slice(&len.to_le_bytes());
            v.extend_from_slice(&(name.len() as u16).to_le_bytes());
            v.extend_from_slice(&0u16.to_le_bytes());
        };
        self.out.extend_from_slice(&0x04034b50u32.to_le_bytes());
        common(&mut self.out);
        self.out.extend_from_slice(name);
        self.out.extend_from_slice(data);
        self.dir.extend_from_slice(&0x02014b50u32.to_le_bytes());
        self.dir.extend_from_slice(&20u16.to_le_bytes());
        common(&mut self.dir);
        // comment length, start disk, internal attributes (2 bytes each), external (4)
        self.dir.extend_from_slice(&[0u8; 10]);
        self.dir.extend_from_slice(&offset.to_le_bytes());
        self.dir.extend_from_slice(name);
        self.count += 1;
    }

    fn finish(mut self) -> Vec<u8> {
        let (start, size) = (self.out.len() as u32, self.dir.len() as u32);
        self.out.extend_from_slice(&self.dir);
        self.out.extend_from_slice(&0x06054b50u32.to_le_bytes());
        self.out.extend_from_slice(&[0u8; 4]);
        self.out.extend_from_slice(&self.count.to_le_bytes());
        self.out.extend_from_slice(&self.count.to_le_bytes());
        self.out.extend_from_slice(&size.to_le_bytes());
        self.out.extend_from_slice(&start.to_le_bytes());
        self.out.extend_from_slice(&0u16.to_le_bytes());
        self.out
    }
}

fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            // characters XML 1.0 does not allow
            c if (c as u32) < 0x20 && !matches!(c, '\t' | '\n' | '\r') => {}
            c => o.push(c),
        }
    }
    o
}

/// Column letters: 0 → A, 25 → Z, 26 → AA.
fn col_name(mut i: usize) -> String {
    let mut s = Vec::new();
    loop {
        s.push(b'A' + (i % 26) as u8);
        if i < 26 {
            break;
        }
        i = i / 26 - 1;
    }
    s.reverse();
    String::from_utf8(s).unwrap()
}

/// A plain decimal number ("3.97", "-4", "12,5"), but not codes such as
/// "007" or "1e5" that should stay text.
fn as_number(s: &str) -> Option<f64> {
    let t = s.trim().replace(',', ".");
    let digits = t.strip_prefix('-').unwrap_or(&t);
    let ok = !digits.is_empty()
        && digits.chars().all(|c| c.is_ascii_digit() || c == '.')
        && digits.chars().filter(|&c| c == '.').count() <= 1
        && !digits.starts_with('.') && !digits.ends_with('.')
        && !(digits.len() > 1 && digits.starts_with('0') && !digits.starts_with("0."));
    if ok { t.parse().ok().filter(|v: &f64| v.is_finite()) } else { None }
}

/// Sheet names: at most 31 characters, without []:*?/\, unique.
fn sheet_names(names: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for (i, n) in names.iter().enumerate() {
        let clean: String = n.chars().filter(|c| !"[]:*?/\\".contains(*c)).collect();
        let clean = clean.trim().trim_matches('\'').to_string();
        let base: String = if clean.is_empty() { format!("Sheet{}", i + 1) } else { clean.chars().take(31).collect() };
        let mut name = base.clone();
        let mut k = 2;
        while out.iter().any(|o| o.to_lowercase() == name.to_lowercase()) {
            let suffix = format!(" ({k})");
            name = base.chars().take(31 - suffix.chars().count()).collect::<String>() + &suffix;
            k += 1;
        }
        out.push(name);
    }
    out
}

fn sheet_xml(header: &[Value], rows: &[Value]) -> String {
    let ncol = header.len().max(1);
    let mut width = vec![8usize; ncol];
    let text = |v: &Value| match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    };
    let mut body = String::new();
    let mut row_xml = |r: usize, cells: &[Value], bold: bool, body: &mut String| {
        body.push_str(&format!("<row r=\"{r}\">"));
        for (c, v) in cells.iter().enumerate().take(ncol) {
            let reference = format!("{}{r}", col_name(c));
            let style = if bold { " s=\"1\"" } else { "" };
            let shown = text(v);
            if c < width.len() {
                width[c] = width[c].max(shown.chars().count().min(60) + 2);
            }
            let number = match v {
                Value::Number(n) if !bold => n.as_f64(),
                Value::String(s) if !bold => as_number(s),
                _ => None,
            };
            if let Some(x) = number {
                body.push_str(&format!("<c r=\"{reference}\"{style}><v>{x}</v></c>"));
            } else if !shown.is_empty() && shown != "–" {
                body.push_str(&format!("<c r=\"{reference}\" t=\"inlineStr\"{style}><is><t xml:space=\"preserve\">{}</t></is></c>",
                                       esc(&shown)));
            }
        }
        body.push_str("</row>");
    };
    row_xml(1, header, true, &mut body);
    for (i, r) in rows.iter().enumerate() {
        if let Some(cells) = r.as_array() {
            row_xml(i + 2, cells, false, &mut body);
        }
    }
    let last = format!("{}{}", col_name(ncol - 1), rows.len() + 1);
    let cols: String = width.iter().enumerate()
        .map(|(i, w)| format!("<col min=\"{0}\" max=\"{0}\" width=\"{w}\" customWidth=\"1\"/>", i + 1)).collect();
    format!(concat!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n",
        "<worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\">",
        "<sheetViews><sheetView workbookViewId=\"0\"><pane ySplit=\"1\" topLeftCell=\"A2\" activePane=\"bottomLeft\" state=\"frozen\"/></sheetView></sheetViews>",
        "<cols>{}</cols><sheetData>{}</sheetData><autoFilter ref=\"A1:{}\"/></worksheet>"), cols, body, last)
}

/// `{"blaetter": [{"name", "spalten": [...], "zeilen": [[...], ...]}]}` → .xlsx bytes.
pub fn xlsx(req: &Value) -> Vec<u8> {
    let sheets: Vec<Value> = req.get("blaetter").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    let sheets = if sheets.is_empty() { vec![serde_json::json!({"name": "Sheet1", "spalten": [], "zeilen": []})] } else { sheets };
    let names = sheet_names(&sheets.iter().map(|s| s.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string()).collect::<Vec<_>>());
    let empty = Vec::new();
    let mut zip = Zip::default();
    let mut put = |name: &str, content: &str| zip.put(name, content.as_bytes());
    let head = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n";
    let overrides: String = (1..=sheets.len()).map(|i| format!(
        "<Override PartName=\"/xl/worksheets/sheet{i}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/>")).collect();
    put("[Content_Types].xml", &format!(concat!("{}<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">",
        "<Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>",
        "<Default Extension=\"xml\" ContentType=\"application/xml\"/>",
        "<Override PartName=\"/xl/workbook.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/>",
        "<Override PartName=\"/xl/styles.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml\"/>",
        "{}</Types>"), head, overrides));
    put("_rels/.rels", &format!(concat!("{}<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">",
        "<Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"xl/workbook.xml\"/>",
        "</Relationships>"), head));
    let sheet_list: String = names.iter().enumerate().map(|(i, n)| format!(
        "<sheet name=\"{}\" sheetId=\"{}\" r:id=\"rId{}\"/>", esc(n), i + 1, i + 1)).collect();
    let filters: String = sheets.iter().zip(&names).enumerate().map(|(i, (s, n))| {
        let ncol = s.get("spalten").and_then(|v| v.as_array()).map_or(1, |a| a.len().max(1));
        let nrow = s.get("zeilen").and_then(|v| v.as_array()).map_or(0, |a| a.len()) + 1;
        format!("<definedName name=\"_xlnm._FilterDatabase\" localSheetId=\"{i}\" hidden=\"1\">'{}'!$A$1:${}${nrow}</definedName>",
                esc(&n.replace('\'', "''")), col_name(ncol - 1))
    }).collect();
    put("xl/workbook.xml", &format!(concat!("{}<workbook xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" ",
        "xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\">",
        "<sheets>{}</sheets><definedNames>{}</definedNames></workbook>"), head, sheet_list, filters));
    let rels: String = (1..=sheets.len()).map(|i| format!(
        "<Relationship Id=\"rId{i}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet\" Target=\"worksheets/sheet{i}.xml\"/>")).collect();
    put("xl/_rels/workbook.xml.rels", &format!(concat!("{}<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">",
        "{}<Relationship Id=\"rId{}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles\" Target=\"styles.xml\"/>",
        "</Relationships>"), head, rels, sheets.len() + 1));
    put("xl/styles.xml", &format!(concat!("{}<styleSheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\">",
        "<fonts count=\"2\"><font><sz val=\"11\"/><name val=\"Calibri\"/></font><font><b/><sz val=\"11\"/><name val=\"Calibri\"/></font></fonts>",
        "<fills count=\"2\"><fill><patternFill patternType=\"none\"/></fill><fill><patternFill patternType=\"gray125\"/></fill></fills>",
        "<borders count=\"1\"><border><left/><right/><top/><bottom/><diagonal/></border></borders>",
        "<cellStyleXfs count=\"1\"><xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\"/></cellStyleXfs>",
        "<cellXfs count=\"2\"><xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\" xfId=\"0\"/>",
        "<xf numFmtId=\"0\" fontId=\"1\" fillId=\"0\" borderId=\"0\" xfId=\"0\" applyFont=\"1\"/></cellXfs>",
        "<cellStyles count=\"1\"><cellStyle name=\"Normal\" xfId=\"0\" builtinId=\"0\"/></cellStyles>",
        "</styleSheet>"), head));
    for (i, s) in sheets.iter().enumerate() {
        let header = s.get("spalten").and_then(|v| v.as_array()).unwrap_or(&empty);
        let rows = s.get("zeilen").and_then(|v| v.as_array()).unwrap_or(&empty);
        put(&format!("xl/worksheets/sheet{}.xml", i + 1), &sheet_xml(header, rows));
    }
    zip.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_numbers() {
        assert_eq!(col_name(0), "A");
        assert_eq!(col_name(25), "Z");
        assert_eq!(col_name(26), "AA");
        assert_eq!(col_name(27), "AB");
        assert_eq!(as_number("3.9692"), Some(3.9692));
        assert_eq!(as_number("12,5"), Some(12.5));
        assert_eq!(as_number("-4"), Some(-4.0));
        assert_eq!(as_number("0.5"), Some(0.5));
        for s in ["007", "1e5", "600 : 800", "P-1", "", "–", "1.2.3", ".5"] {
            assert_eq!(as_number(s), None, "{s}");
        }
        let n = sheet_names(&["a/b".into(), "A/B".into(), "".into(), "x".repeat(40)]);
        assert_eq!(n, ["ab", "AB (2)", "Sheet3", &"x".repeat(31)]);
    }
}
