//! Generate test PDFs for BDD tests.
//!
//! Run: cargo run --example generate_test_pdfs

use std::io::Write;

use lopdf::{content::Content, content::Operation, Document, Object, Stream, dictionary, object::ObjectId};

fn main() {
    // Generate page_1.pdf
    let mut doc1 = Document::with_version("1.4");

    // Create a simple page with "Page 1" text
    let page_id = doc1.new_object_id();
    let content_id = doc1.new_object_id();

    // Create content stream with text
    let mut content = Content::new(vec![]);

    // BT = Begin Text, ET = End Text
    // Tf = Set font, Td = Move text position, Tj = Show text
    content.operations.push(Operation::new("BT", vec![]));
    content.operations.push(Operation::new(
        "Tf",
        vec![Object::Name(b"Helvetica".to_vec()), Object::Real(24.0)],
    ));
    content.operations.push(Operation::new(
        "Td",
        vec![Object::Real(100.0), Object::Real(700.0)],
    ));
    content.operations.push(Operation::new(
        "Tj",
        vec![Object::String(b"Page 1".to_vec(), lopdf::StringFormat::Literal)],
    ));
    content.operations.push(Operation::new("ET", vec![]));

    let content_stream = Stream::new(
        dictionary! {
            "Length" => Object::Integer(content.encode().unwrap().len() as i64),
        },
        content.encode().unwrap(),
    );

    doc1.objects.insert(content_id, Object::Stream(content_stream));

    // Create page dictionary
    let page_dict = dictionary! {
        "Type" => Object::Name(b"Page".to_vec()),
        "Parent" => Object::Reference(doc1.new_object_id()), // Will fix later
        "MediaBox" => Object::Array(vec![
            Object::Integer(0), Object::Integer(0),
            Object::Integer(612), Object::Integer(792), // Letter size
        ]),
        "Contents" => Object::Reference(content_id),
    };

    doc1.objects.insert(page_id, Object::Dictionary(page_dict));

    // Save
    let mut output1 = Vec::new();
    doc1.save_to(&mut output1).unwrap();
    std::fs::write("tests/bdd/testdata/page_1.pdf", &output1).unwrap();
    println!("Generated page_1.pdf ({} bytes)", output1.len());

    // Generate page_2.pdf (similar but with "Page 2")
    let mut doc2 = Document::with_version("1.4");
    let page_id2 = doc2.new_object_id();
    let content_id2 = doc2.new_object_id();

    let mut content2 = Content::new(vec![]);
    content2.operations.push(Operation::new("BT", vec![]));
    content2.operations.push(Operation::new(
        "Tf",
        vec![Object::Name(b"Helvetica".to_vec()), Object::Real(24.0)],
    ));
    content2.operations.push(Operation::new(
        "Td",
        vec![Object::Real(100.0), Object::Real(700.0)],
    ));
    content2.operations.push(Operation::new(
        "Tj",
        vec![Object::String(b"Page 2".to_vec(), lopdf::StringFormat::Literal)],
    ));
    content2.operations.push(Operation::new("ET", vec![]));

    let content_stream2 = Stream::new(
        dictionary! {
            "Length" => Object::Integer(content2.encode().unwrap().len() as i64),
        },
        content2.encode().unwrap(),
    );

    doc2.objects.insert(content_id2, Object::Stream(content_stream2));

    let page_dict2 = dictionary! {
        "Type" => Object::Name(b"Page".to_vec()),
        "Parent" => Object::Reference(doc2.new_object_id()),
        "MediaBox" => Object::Array(vec![
            Object::Integer(0), Object::Integer(0),
            Object::Integer(612), Object::Integer(792),
        ]),
        "Contents" => Object::Reference(content_id2),
    };

    doc2.objects.insert(page_id2, Object::Dictionary(page_dict2));

    let mut output2 = Vec::new();
    doc2.save_to(&mut output2).unwrap();
    std::fs::write("tests/bdd/testdata/page_2.pdf", &output2).unwrap();
    println!("Generated page_2.pdf ({} bytes)", output2.len());

    println!("Test PDFs generated successfully!");
}
