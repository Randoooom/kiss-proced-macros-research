// This line is required for the derive macros, because they just assume
// we already imported them globally
#[macro_use]
extern crate serde;
#[macro_use]
extern crate codegen;
#[macro_use]
extern crate serde_json;

/// This is the basic trait, which is gonna be autoimplemented
/// by our created 'ReverseFlat' macro
trait ReverseFlat {
    fn reverse(value: serde_json::Value) -> Result<Self, serde_json::Error>
    where
        Self: Sized;
}

#[test]
fn normal() {
    #[derive(ReverseFlat)]
    struct Data {
        content: String,
    }
    let data = json!({ "content": "Hello, world!" });

    let processed = Data::reverse(data);
    assert!(processed.is_ok());
    assert_eq!(processed.unwrap().content, "Hello, world!");
}

#[test]
fn simple_flat() {
    #[derive(ReverseFlat)]
    struct Flatted {
        content: String,
    }

    #[derive(ReverseFlat)]
    struct Data {
        #[reverse(prefix = "flatted")]
        flatted: Flatted,
    }
    let data = json!({ "flatted_content": "Hello, world!" });

    let processed = Data::reverse(data);
    assert!(processed.is_ok());
    assert_eq!(processed.unwrap().flatted.content, "Hello, world!");
}

#[test]
fn flat_with_root_properties() {
    #[derive(ReverseFlat)]
    struct Flatted {
        content: String,
    }

    #[derive(ReverseFlat)]
    struct Data {
        #[reverse(prefix = "flatted")]
        flatted: Flatted,
        root: String,
    }
    let data = json!({ "flatted_content": "Hello, world!", "root": "Hello, root!" });

    let processed = Data::reverse(data);
    assert!(processed.is_ok());
    let processed = processed.unwrap();

    assert_eq!(processed.flatted.content, "Hello, world!");
    assert_eq!(processed.root, "Hello, root!");
}

#[test]
fn flat_recursive() {
    #[derive(ReverseFlat)]
    struct Second {
        message: String,
    }

    #[derive(ReverseFlat)]
    struct Flatted {
        #[reverse(prefix = "content")]
        content: Second,
    }

    #[derive(ReverseFlat)]
    struct Data {
        #[reverse(prefix = "flatted")]
        flatted: Flatted,
    }
    let data = json!({ "flatted_content_message": "Hello, world!" });

    let processed = Data::reverse(data);
    assert!(processed.is_ok());
    assert_eq!(processed.unwrap().flatted.content.message, "Hello, world!");
}
