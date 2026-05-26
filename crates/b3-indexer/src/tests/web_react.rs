use super::*;

#[test]
fn detects_react_tsx_components_props_hooks_and_usages() {
    let parsed = WebLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("react"),
            path: PathBuf::from("src/ProductCard.tsx"),
            source: r#"
                    import React, { useEffect, useState, memo } from "react";

                    interface ProductCardProps {
                        name: string;
                    }

                    type BadgeProps = {
                        label: string;
                    };

                    export function ProductCard(props: ProductCardProps) {
                        const [open, setOpen] = useState(false);
                        useEffect(() => {}, []);
                        return <Badge label={props.name} />;
                    }

                    const Badge = ({ label }: BadgeProps) => <span>{label}</span>;
                    export default memo(ProductCard);

                    function helper() {
                        return "not jsx";
                    }
                "#
            .to_string(),
        })
        .expect("parse react tsx");

    let product = parsed
        .symbols
        .iter()
        .find(|symbol| symbol.name == "ProductCard")
        .expect("ProductCard symbol");
    let product_metadata = product.visibility.as_deref().unwrap_or_default();
    assert_eq!(
        component_metadata_value(product_metadata, "framework").as_deref(),
        Some("react")
    );
    assert_eq!(
        component_metadata_value(product_metadata, "props").as_deref(),
        Some("ProductCardProps")
    );
    assert!(component_metadata_value(product_metadata, "hooks")
        .unwrap_or_default()
        .contains("useState"));
    assert!(component_metadata_value(product_metadata, "usages")
        .unwrap_or_default()
        .contains("Badge"));

    let badge = parsed
        .symbols
        .iter()
        .find(|symbol| symbol.name == "Badge")
        .expect("Badge symbol");
    assert_eq!(
        component_metadata_value(badge.visibility.as_deref().unwrap_or_default(), "props")
            .as_deref(),
        Some("BadgeProps")
    );
    assert!(!parsed.symbols.iter().any(|symbol| {
        symbol.name == "helper"
            && component_metadata_value(
                symbol.visibility.as_deref().unwrap_or_default(),
                "framework",
            )
            .is_some()
    }));
    assert!(parsed
        .relationships
        .iter()
        .any(|edge| edge.kind == EdgeKind::References));
}

#[test]
fn detects_react_jsx_components_and_class_components() {
    let parsed = WebLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("jsx"),
            path: PathBuf::from("src/App.jsx"),
            source: r#"
                    import * as React from "react";

                    class ProductCard extends React.Component {
                        render() {
                            return <section />;
                        }
                    }

                    export const App = () => <ProductCard />;
                    const value = () => "plain";
                "#
            .to_string(),
        })
        .expect("parse react jsx");

    let app = parsed
        .symbols
        .iter()
        .find(|symbol| symbol.name == "App")
        .expect("App symbol");
    assert_eq!(
        component_metadata_value(app.visibility.as_deref().unwrap_or_default(), "framework")
            .as_deref(),
        Some("react")
    );
    let product = parsed
        .symbols
        .iter()
        .find(|symbol| symbol.name == "ProductCard")
        .expect("ProductCard symbol");
    assert_eq!(
        component_metadata_value(product.visibility.as_deref().unwrap_or_default(), "kind")
            .as_deref(),
        Some("class")
    );
    assert!(!parsed.symbols.iter().any(|symbol| {
        symbol.name == "value"
            && component_metadata_value(
                symbol.visibility.as_deref().unwrap_or_default(),
                "framework",
            )
            .is_some()
    }));
}

#[test]
fn detects_nextjs_use_client_and_server_component_classification() {
    let client = WebLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("client"),
            path: PathBuf::from("app/users/UserPanel.tsx"),
            source: r#"
                    "use client";
                    export function UserPanel() {
                        return <section />;
                    }
                "#
            .to_string(),
        })
        .expect("parse client component");
    let client_component = client
        .symbols
        .iter()
        .find(|symbol| symbol.name == "UserPanel")
        .expect("client component");
    let client_metadata = client_component.visibility.as_deref().unwrap_or_default();
    assert_eq!(
        component_metadata_value(client_metadata, "framework").as_deref(),
        Some("nextjs")
    );
    assert_eq!(
        component_metadata_value(client_metadata, "kind").as_deref(),
        Some("client_component")
    );

    let server = WebLanguagePack
        .parse(ParseInput {
            file_id: FileId::new("server"),
            path: PathBuf::from("app/users/UserList.tsx"),
            source: r#"
                    export default function UserList() {
                        return <section />;
                    }
                "#
            .to_string(),
        })
        .expect("parse server component");
    let server_component = server
        .symbols
        .iter()
        .find(|symbol| symbol.name == "UserList")
        .expect("server component");
    assert_eq!(
        component_metadata_value(
            server_component.visibility.as_deref().unwrap_or_default(),
            "kind"
        )
        .as_deref(),
        Some("server_component")
    );
}
