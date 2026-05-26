use super::*;

pub(crate) fn collect_threejs_webgl(input: &ParseInput) -> Vec<ExtractedSymbol> {
    let mut symbols = Vec::new();
    let source = input.source.as_str();
    let has_three = source.contains("from 'three'")
        || source.contains("from \"three\"")
        || source.contains("@react-three/fiber")
        || source.contains("@react-three/drei");
    let has_webgl = source.contains("getContext(\"webgl\")")
        || source.contains("getContext('webgl')")
        || source.contains("getContext(\"webgl2\")")
        || source.contains("getContext('webgl2')")
        || source.contains("WebGLRenderingContext");
    if !(has_three || has_webgl) {
        return symbols;
    }
    for (index, line) in input.source.lines().enumerate() {
        let line_number = index + 1;
        for keyword in [
            "Scene",
            "Camera",
            "Renderer",
            "Mesh",
            "Light",
            "AmbientLight",
            "DirectionalLight",
            "Geometry",
            "Material",
            "ShaderMaterial",
            "TextureLoader",
            "GLTFLoader",
            "WebGLRenderer",
            "requestAnimationFrame",
        ] {
            if line.contains(keyword) {
                symbols.push(symbol(
                    input,
                    "threejs_webgl",
                    keyword,
                    NodeKind::ConfigKey,
                    line_number,
                    format!(
                        "webgl.hint={keyword};webgl.technology={};webgl.file={}",
                        if has_three { "threejs" } else { "webgl" },
                        normalized_file(input)
                    ),
                ));
            }
        }
        for asset in asset_literals(line) {
            symbols.push(symbol(
                input,
                "threejs_webgl",
                asset.clone(),
                NodeKind::Package,
                line_number,
                format!("webgl.asset={asset};webgl.file={}", normalized_file(input)),
            ));
        }
        if let Some(canvas_id) = canvas_id(line) {
            symbols.push(symbol(
                input,
                "threejs_webgl",
                canvas_id.clone(),
                NodeKind::ConfigKey,
                line_number,
                format!(
                    "webgl.canvas_id={canvas_id};webgl.file={}",
                    normalized_file(input)
                ),
            ));
        }
    }
    symbols
}

fn asset_literals(line: &str) -> Vec<String> {
    let mut values = Vec::new();
    for quote in ['"', '\''] {
        let mut rest = line;
        while let Some(start) = rest.find(quote) {
            rest = &rest[start + 1..];
            let Some(end) = rest.find(quote) else {
                break;
            };
            let value = &rest[..end];
            if [
                ".glb", ".gltf", ".obj", ".fbx", ".png", ".jpg", ".jpeg", ".webp", ".glsl",
                ".vert", ".frag",
            ]
            .iter()
            .any(|suffix| value.ends_with(suffix))
            {
                values.push(value.to_string());
            }
            rest = &rest[end + 1..];
        }
    }
    values
}

fn canvas_id(line: &str) -> Option<String> {
    for marker in ["getElementById(", "querySelector("] {
        if let Some(index) = line.find(marker) {
            let rest = &line[index + marker.len()..];
            let value = super::literal_after(rest, "")?;
            return Some(value.trim_start_matches('#').to_string());
        }
    }
    None
}
