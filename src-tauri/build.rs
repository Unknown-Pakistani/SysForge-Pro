fn main() {
    let mut attributes = tauri_build::Attributes::new();

    #[cfg(windows)]
    {
        attributes = attributes.windows_attributes(
            tauri_build::WindowsAttributes::new()
                .app_manifest(include_str!("app-manifest.xml"))
        );
    }

    tauri_build::try_build(attributes).expect("failed to build Tauri app");
}
