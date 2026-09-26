
use winresource::{VersionInfo, WindowsResource};

const MANIFEST: &str = r#"
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="asInvoker" uiAccess="false"/>
      </requestedPrivileges>
    </security>
  </trustInfo>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}"/>
    </application>
  </compatibility>
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <dpiAware xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">true/pm</dpiAware>
      <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">PerMonitorV2, PerMonitor</dpiAwareness>
    </windowsSettings>
  </application>
</assembly>
"#;

fn build_date() -> String {
    println!("cargo:rerun-if-env-changed=BUILD_DATE");
    std::env::var("BUILD_DATE").unwrap_or_else(|_| today_utc())
}

fn today_utc() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let z = (secs / 86_400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

fn windows_version(date: &str) -> (u64, String) {
    let parts: Vec<u64> = date.split('-').filter_map(|p| p.parse().ok()).collect();
    let &[y, m, d] = parts.as_slice() else { panic!("BUILD_DATE invalide : {date} (attendu AAAA-MM-JJ)") };
    (y << 48 | m << 32 | d << 16, format!("{y}.{m}.{d}.0"))
}

fn main() {
    println!("cargo:rerun-if-changed=Icon.ico");
    let date = build_date();
    let (version, version_text) = windows_version(&date);
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        WindowsResource::new()
            .set_icon("Icon.ico")
            .set("FileDescription", "CASPER Shot")
            .set("ProductName", "CASPER Shot")
            .set("OriginalFilename", "casper_shot.exe")
            .set("InternalName", "casper_shot")
            .set("CompanyName", "Nyaldee")
            .set("LegalCopyright", "Copyright © 2026 Nyaldee")
            .set("FileVersion", &version_text)
            .set("ProductVersion", &date.replace('-', "."))
            .set_version_info(VersionInfo::FILEVERSION, version)
            .set_version_info(VersionInfo::PRODUCTVERSION, version)
            .set_manifest(MANIFEST)
            .compile()
            .expect("échec de l'embarquement des ressources Windows");
    }
}
