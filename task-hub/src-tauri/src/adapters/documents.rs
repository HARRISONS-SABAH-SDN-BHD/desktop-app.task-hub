//! Small, dependency-light OOXML exporters. Each format owns its package structure.
use crate::model::Result;
use std::{
    collections::BTreeMap,
    io::{Cursor, Write},
};
use zip::{write::SimpleFileOptions, ZipWriter};

const XML: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>";
const REL_NS: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const OFFICE_REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
pub fn xml(value: &str) -> String {
    value
        .chars()
        .filter(|c| !c.is_control() || matches!(*c, '\t' | '\n' | '\r'))
        .collect::<String>()
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
pub fn csv_rows(body: &str) -> Result<Vec<Vec<String>>> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(false)
        .from_reader(body.as_bytes());
    let mut rows = Vec::new();
    let mut cells = 0;
    for record in reader.records() {
        let record = record.map_err(|e| format!("Invalid CSV: {e}"))?;
        cells += record.len();
        if rows.len() >= 5000 || record.len() > 200 || cells > 10000 {
            return Err("CSV limit: 5,000 rows, 200 columns, 10,000 cells".into());
        }
        rows.push(record.iter().map(String::from).collect());
    }
    if rows.is_empty() {
        return Err("CSV data is empty".into());
    }
    Ok(rows)
}
struct Package {
    parts: Vec<(String, String)>,
    overrides: Vec<(String, String)>,
}
impl Package {
    fn new(main: &str, title: &str) -> Self {
        let mut package = Self {
            parts: Vec::new(),
            overrides: Vec::new(),
        };
        package.add("_rels/.rels", None, rels(&[("rId1", &format!("{OFFICE_REL}/officeDocument"), main), ("rId2", "http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties", "docProps/core.xml")]));
        package.add("docProps/core.xml", Some("application/vnd.openxmlformats-package.core-properties+xml"), format!("{XML}<cp:coreProperties xmlns:cp=\"http://schemas.openxmlformats.org/package/2006/metadata/core-properties\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\"><dc:title>{}</dc:title><dc:creator>Task Hub</dc:creator></cp:coreProperties>", xml(title)));
        package
    }
    fn add(&mut self, path: &str, mime: Option<&str>, content: String) {
        self.parts.push((path.into(), content));
        if let Some(mime) = mime {
            self.overrides.push((path.into(), mime.into()));
        }
    }
    fn finish(mut self) -> Result<Vec<u8>> {
        let overrides = self
            .overrides
            .iter()
            .map(|(path, mime)| format!("<Override PartName=\"/{path}\" ContentType=\"{mime}\"/>"))
            .collect::<String>();
        self.parts.push(("[Content_Types].xml".into(), format!("{XML}<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/>{overrides}</Types>")));
        let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
        for (name, content) in self.parts {
            zip.start_file(
                name,
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated),
            )
            .map_err(|e| e.to_string())?;
            zip.write_all(content.as_bytes())
                .map_err(|e| e.to_string())?;
        }
        Ok(zip.finish().map_err(|e| e.to_string())?.into_inner())
    }
}
fn rels(entries: &[(&str, &str, &str)]) -> String {
    format!(
        "{XML}<Relationships xmlns=\"{REL_NS}\">{}</Relationships>",
        entries
            .iter()
            .map(|(id, kind, target)| format!(
                "<Relationship Id=\"{id}\" Type=\"{kind}\" Target=\"{target}\"/>"
            ))
            .collect::<String>()
    )
}
pub fn create(format: &str, fields: &BTreeMap<String, String>) -> Result<Vec<u8>> {
    match format {
        "docx" => word(&fields["title"], &fields["body"]),
        "xlsx" => excel(&fields["title"], &fields["body"]),
        "pptx" => powerpoint(&fields["title"], &fields["body"]),
        _ => Err("Unsupported Office format".into()),
    }
}
fn word(title: &str, body: &str) -> Result<Vec<u8>> {
    let mut package = Package::new("word/document.xml", title);
    let paragraphs = body.replace("\r\n", "\n").split('\n').map(|line| format!("<w:p><w:pPr><w:spacing w:after=\"160\"/></w:pPr><w:r><w:t xml:space=\"preserve\">{}</w:t></w:r></w:p>", xml(line))).collect::<String>();
    package.add("word/document.xml", Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"), format!("{XML}<w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:body><w:p><w:pPr><w:pStyle w:val=\"Title\"/></w:pPr><w:r><w:t>{}</w:t></w:r></w:p>{paragraphs}<w:sectPr><w:pgSz w:w=\"11906\" w:h=\"16838\"/><w:pgMar w:top=\"1134\" w:right=\"1134\" w:bottom=\"1134\" w:left=\"1134\" w:header=\"708\" w:footer=\"708\" w:gutter=\"0\"/></w:sectPr></w:body></w:document>", xml(title)));
    package.add("word/styles.xml", Some("application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"), format!("{XML}<w:styles xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii=\"Calibri\" w:hAnsi=\"Calibri\"/><w:color w:val=\"27364A\"/><w:sz w:val=\"22\"/></w:rPr></w:rPrDefault></w:docDefaults><w:style w:type=\"paragraph\" w:default=\"1\" w:styleId=\"Normal\"><w:name w:val=\"Normal\"/></w:style><w:style w:type=\"paragraph\" w:styleId=\"Title\"><w:name w:val=\"Title\"/><w:basedOn w:val=\"Normal\"/><w:pPr><w:spacing w:after=\"360\"/></w:pPr><w:rPr><w:b/><w:color w:val=\"174A50\"/><w:sz w:val=\"48\"/></w:rPr></w:style></w:styles>"));
    package.add(
        "word/_rels/document.xml.rels",
        None,
        rels(&[("rId1", &format!("{OFFICE_REL}/styles"), "styles.xml")]),
    );
    package.finish()
}
fn column(mut index: usize) -> String {
    let mut result = String::new();
    while index > 0 {
        index -= 1;
        result.insert(0, (b'A' + (index % 26) as u8) as char);
        index /= 26;
    }
    result
}
fn excel(title: &str, body: &str) -> Result<Vec<u8>> {
    let rows = csv_rows(body)?;
    let mut package = Package::new("xl/workbook.xml", title);
    package.add("xl/workbook.xml", Some("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"), format!("{XML}<workbook xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" xmlns:r=\"{OFFICE_REL}\"><sheets><sheet name=\"Report\" sheetId=\"1\" r:id=\"rId1\"/></sheets></workbook>"));
    let data = rows.iter().enumerate().map(|(r, row)| {
        let cells = row.iter().enumerate().map(|(c, value)| format!("<c r=\"{}{}\" t=\"inlineStr\" s=\"{}\"><is><t xml:space=\"preserve\">{}</t></is></c>", column(c + 1), r + 1, if r == 0 { 1 } else { 0 }, xml(value))).collect::<String>();
        format!("<row r=\"{}\">{cells}</row>", r + 1)
    }).collect::<String>();
    let last = format!("{}{}", column(rows[0].len()), rows.len());
    package.add("xl/worksheets/sheet1.xml", Some("application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"), format!("{XML}<worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><dimension ref=\"A1:{last}\"/><sheetViews><sheetView workbookViewId=\"0\"><pane ySplit=\"1\" topLeftCell=\"A2\" activePane=\"bottomLeft\" state=\"frozen\"/></sheetView></sheetViews><sheetFormatPr defaultRowHeight=\"20\"/><cols><col min=\"1\" max=\"{}\" width=\"28\" customWidth=\"1\"/></cols><sheetData>{data}</sheetData><autoFilter ref=\"A1:{last}\"/></worksheet>", rows[0].len()));
    package.add("xl/styles.xml", Some("application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"), format!("{XML}<styleSheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><fonts count=\"2\"><font><sz val=\"11\"/><name val=\"Calibri\"/></font><font><b/><color rgb=\"FFFFFFFF\"/><sz val=\"11\"/><name val=\"Calibri\"/></font></fonts><fills count=\"3\"><fill><patternFill patternType=\"none\"/></fill><fill><patternFill patternType=\"gray125\"/></fill><fill><patternFill patternType=\"solid\"><fgColor rgb=\"FF174A50\"/><bgColor indexed=\"64\"/></patternFill></fill></fills><borders count=\"1\"><border><left/><right/><top/><bottom/><diagonal/></border></borders><cellStyleXfs count=\"1\"><xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\"/></cellStyleXfs><cellXfs count=\"2\"><xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\" xfId=\"0\"/><xf numFmtId=\"0\" fontId=\"1\" fillId=\"2\" borderId=\"0\" xfId=\"0\" applyFont=\"1\" applyFill=\"1\"/></cellXfs><cellStyles count=\"1\"><cellStyle name=\"Normal\" xfId=\"0\" builtinId=\"0\"/></cellStyles></styleSheet>"));
    package.add(
        "xl/_rels/workbook.xml.rels",
        None,
        rels(&[
            (
                "rId1",
                &format!("{OFFICE_REL}/worksheet"),
                "worksheets/sheet1.xml",
            ),
            ("rId2", &format!("{OFFICE_REL}/styles"), "styles.xml"),
        ]),
    );
    package.finish()
}
const DRAWING_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const PRESENTATION_NS: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";
const GROUP: &str = "<p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"0\" cy=\"0\"/><a:chOff x=\"0\" y=\"0\"/><a:chExt cx=\"0\" cy=\"0\"/></a:xfrm></p:grpSpPr>";
fn text_shape(
    id: u32,
    name: &str,
    lines: &[&str],
    y: u32,
    height: u32,
    size: u32,
    color: &str,
) -> String {
    let paragraphs = lines.iter().map(|line| format!("<a:p><a:pPr marL=\"0\" indent=\"0\"><a:spaceAfter><a:spcPts val=\"1000\"/></a:spaceAfter></a:pPr><a:r><a:rPr lang=\"en-US\" sz=\"{size}\"><a:solidFill><a:srgbClr val=\"{color}\"/></a:solidFill><a:latin typeface=\"Calibri\"/></a:rPr><a:t>{}</a:t></a:r><a:endParaRPr lang=\"en-US\"/></a:p>", xml(line))).collect::<String>();
    format!("<p:sp><p:nvSpPr><p:cNvPr id=\"{id}\" name=\"{name}\"/><p:cNvSpPr txBox=\"1\"/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x=\"731520\" y=\"{y}\"/><a:ext cx=\"10728960\" cy=\"{height}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom><a:noFill/></p:spPr><p:txBody><a:bodyPr wrap=\"square\"><a:normAutofit/></a:bodyPr><a:lstStyle/>{paragraphs}</p:txBody></p:sp>")
}
fn powerpoint(title: &str, body: &str) -> Result<Vec<u8>> {
    let normalized = body.replace("\r\n", "\n");
    let chunks = normalized
        .split("\n---\n")
        .filter(|s| !s.trim().is_empty())
        .collect::<Vec<_>>();
    if chunks.len() > 40
        || title.chars().count() > 160
        || chunks.iter().any(|c| {
            c.chars().count() > 1200
                || c.lines().count() > 13
                || c.lines().next().unwrap_or("").chars().count() > 160
        })
    {
        return Err("Presentation limit: 40 content slides, 160-character headings, 1,200 characters / 13 lines per slide".into());
    }
    let mut slides: Vec<(String, Vec<&str>)> = vec![(title.into(), vec!["Created with Task Hub"])];
    for chunk in chunks {
        let mut lines = chunk.lines();
        slides.push((lines.next().unwrap_or("").into(), lines.collect()));
    }
    let mut package = Package::new("ppt/presentation.xml", title);
    let slide_ids = slides
        .iter()
        .enumerate()
        .map(|(i, _)| format!("<p:sldId id=\"{}\" r:id=\"rId{}\"/>", i + 256, i + 2))
        .collect::<String>();
    package.add("ppt/presentation.xml", Some("application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"), format!("{XML}<p:presentation xmlns:p=\"{PRESENTATION_NS}\" xmlns:a=\"{DRAWING_NS}\" xmlns:r=\"{OFFICE_REL}\"><p:sldMasterIdLst><p:sldMasterId id=\"2147483648\" r:id=\"rId1\"/></p:sldMasterIdLst><p:sldIdLst>{slide_ids}</p:sldIdLst><p:sldSz cx=\"12192000\" cy=\"6858000\" type=\"screen16x9\"/><p:notesSz cx=\"6858000\" cy=\"9144000\"/></p:presentation>"));
    let mut presentation_rels = format!("{XML}<Relationships xmlns=\"{REL_NS}\"><Relationship Id=\"rId1\" Type=\"{OFFICE_REL}/slideMaster\" Target=\"slideMasters/slideMaster1.xml\"/>");
    for (i, (heading, lines)) in slides.iter().enumerate() {
        let n = i + 1;
        presentation_rels.push_str(&format!("<Relationship Id=\"rId{}\" Type=\"{OFFICE_REL}/slide\" Target=\"slides/slide{n}.xml\"/>", i + 2));
        let heading = text_shape(2, "Heading", &[heading], 731520, 1280160, 3800, "174A50");
        let content = text_shape(3, "Content", lines, 2194560, 3657600, 2300, "33475B");
        package.add(&format!("ppt/slides/slide{n}.xml"), Some("application/vnd.openxmlformats-officedocument.presentationml.slide+xml"), format!("{XML}<p:sld xmlns:p=\"{PRESENTATION_NS}\" xmlns:a=\"{DRAWING_NS}\" xmlns:r=\"{OFFICE_REL}\"><p:cSld><p:bg><p:bgPr><a:solidFill><a:srgbClr val=\"F5F9FA\"/></a:solidFill><a:effectLst/></p:bgPr></p:bg><p:spTree>{GROUP}{heading}{content}</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>"));
        package.add(
            &format!("ppt/slides/_rels/slide{n}.xml.rels"),
            None,
            rels(&[(
                "rId1",
                &format!("{OFFICE_REL}/slideLayout"),
                "../slideLayouts/slideLayout1.xml",
            )]),
        );
    }
    presentation_rels.push_str("</Relationships>");
    package.add("ppt/_rels/presentation.xml.rels", None, presentation_rels);
    let clr_map = "<p:clrMap accent1=\"accent1\" accent2=\"accent2\" accent3=\"accent3\" accent4=\"accent4\" accent5=\"accent5\" accent6=\"accent6\" bg1=\"lt1\" bg2=\"lt2\" folHlink=\"folHlink\" hlink=\"hlink\" tx1=\"dk1\" tx2=\"dk2\"/>";
    package.add("ppt/slideMasters/slideMaster1.xml", Some("application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml"), format!("{XML}<p:sldMaster xmlns:p=\"{PRESENTATION_NS}\" xmlns:a=\"{DRAWING_NS}\" xmlns:r=\"{OFFICE_REL}\"><p:cSld><p:spTree>{GROUP}</p:spTree></p:cSld>{clr_map}<p:sldLayoutIdLst><p:sldLayoutId id=\"2147483649\" r:id=\"rId1\"/></p:sldLayoutIdLst><p:txStyles><p:titleStyle/><p:bodyStyle/><p:otherStyle/></p:txStyles></p:sldMaster>"));
    package.add(
        "ppt/slideMasters/_rels/slideMaster1.xml.rels",
        None,
        rels(&[
            (
                "rId1",
                &format!("{OFFICE_REL}/slideLayout"),
                "../slideLayouts/slideLayout1.xml",
            ),
            (
                "rId2",
                &format!("{OFFICE_REL}/theme"),
                "../theme/theme1.xml",
            ),
        ]),
    );
    package.add("ppt/slideLayouts/slideLayout1.xml", Some("application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"), format!("{XML}<p:sldLayout xmlns:p=\"{PRESENTATION_NS}\" xmlns:a=\"{DRAWING_NS}\" xmlns:r=\"{OFFICE_REL}\" type=\"blank\" preserve=\"1\"><p:cSld name=\"Blank\"><p:spTree>{GROUP}</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sldLayout>"));
    package.add(
        "ppt/slideLayouts/_rels/slideLayout1.xml.rels",
        None,
        rels(&[(
            "rId1",
            &format!("{OFFICE_REL}/slideMaster"),
            "../slideMasters/slideMaster1.xml",
        )]),
    );
    let colors = [
        ("dk1", "152C3D"),
        ("lt1", "FFFFFF"),
        ("dk2", "33475B"),
        ("lt2", "F5F9FA"),
        ("accent1", "174A50"),
        ("accent2", "64B6AC"),
        ("accent3", "7998C9"),
        ("accent4", "E4B88E"),
        ("accent5", "B69DD4"),
        ("accent6", "9BD2AD"),
        ("hlink", "276DCC"),
        ("folHlink", "70549A"),
    ]
    .iter()
    .map(|(tag, color)| format!("<a:{tag}><a:srgbClr val=\"{color}\"/></a:{tag}>"))
    .collect::<String>();
    let fill = "<a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill>";
    let line = format!("<a:ln w=\"9525\">{fill}<a:prstDash val=\"solid\"/></a:ln>");
    package.add("ppt/theme/theme1.xml", Some("application/vnd.openxmlformats-officedocument.theme+xml"), format!("{XML}<a:theme xmlns:a=\"{DRAWING_NS}\" name=\"Task Hub\"><a:themeElements><a:clrScheme name=\"Task Hub\">{colors}</a:clrScheme><a:fontScheme name=\"Calibri\"><a:majorFont><a:latin typeface=\"Calibri\"/><a:ea typeface=\"\"/><a:cs typeface=\"\"/></a:majorFont><a:minorFont><a:latin typeface=\"Calibri\"/><a:ea typeface=\"\"/><a:cs typeface=\"\"/></a:minorFont></a:fontScheme><a:fmtScheme name=\"Task Hub\"><a:fillStyleLst>{fill}{fill}{fill}</a:fillStyleLst><a:lnStyleLst>{line}{line}{line}</a:lnStyleLst><a:effectStyleLst><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle></a:effectStyleLst><a:bgFillStyleLst>{fill}{fill}{fill}</a:bgFillStyleLst></a:fmtScheme></a:themeElements></a:theme>"));
    package.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    #[test]
    fn office_packages_have_valid_xml_and_relationship_targets() {
        for format in ["docx", "xlsx", "pptx"] {
            let fields = BTreeMap::from([
                ("title".into(), "Quarterly <Review> & 日本語".into()),
                (
                    "body".into(),
                    if format == "xlsx" {
                        "Name,Value\n\"Comma, here\",=1+1"
                    } else {
                        "Overview\nHello & world\n---\nNext steps\nFinish"
                    }
                    .into(),
                ),
            ]);
            let bytes = create(format, &fields).unwrap();
            let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
            let names = (0..zip.len())
                .map(|i| zip.by_index(i).unwrap().name().to_string())
                .collect::<Vec<_>>();
            for name in &names {
                let mut text = String::new();
                zip.by_name(name)
                    .unwrap()
                    .read_to_string(&mut text)
                    .unwrap();
                let document = roxmltree::Document::parse(&text).unwrap();
                if name.ends_with(".rels") {
                    let base = if name == "_rels/.rels" {
                        String::new()
                    } else {
                        format!("{}/", name.split("/_rels/").next().unwrap())
                    };
                    for node in document
                        .descendants()
                        .filter(|n| n.is_element() && n.tag_name().name() == "Relationship")
                    {
                        let mut pieces = Vec::new();
                        let path = format!("{base}{}", node.attribute("Target").unwrap());
                        for p in path.split('/') {
                            if p == ".." {
                                pieces.pop();
                            } else if !p.is_empty() {
                                pieces.push(p);
                            }
                        }
                        assert!(
                            names.contains(&pieces.join("/")),
                            "Missing relationship target: {path}"
                        );
                    }
                }
                if name.ends_with("sheet1.xml") {
                    assert!(!text.contains("<f>"));
                    assert!(text.contains("=1+1"));
                }
            }
        }
    }
    #[test]
    fn csv_quoted_fields_and_invalid_rows() {
        assert_eq!(csv_rows("A,B\n\"x,y\",2").unwrap()[1][0], "x,y");
        assert!(csv_rows("A,B\nx").is_err());
    }
    #[test]
    fn column_names() {
        assert_eq!(column(1), "A");
        assert_eq!(column(26), "Z");
        assert_eq!(column(27), "AA");
    }
}
