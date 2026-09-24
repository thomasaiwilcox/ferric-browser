use std::{env, fmt::Write as _, fs, path::Path, path::PathBuf};

use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    let qml_files = collect_qml_files(Path::new("qml"));
    let resources = collect_browser_resources(Path::new("qml"));
    let manifest_path = write_script_manifest(&resources);
    let qrc_path = manifest_path.with_file_name("browser_script_manifest.qrc");
    write_manifest_qrc(&qrc_path, &manifest_path, &resources);

    CxxQtBuilder::new_qml_module(QmlModule::new("io.github.ferricbrowser").qml_files(qml_files))
        .files(["src/lib.rs"])
        .cpp_files([
            "src/browser_key_router.h",
            "src/browser_key_router.cpp",
            "src/request_interceptor.h",
            "src/request_interceptor.cpp",
            "src/blocklist_updater.h",
            "src/blocklist_updater.cpp",
            "src/link_rule_updater.h",
            "src/link_rule_updater.cpp",
            "src/notification_presenter.h",
            "src/notification_presenter.cpp",
            "src/mpris_controller.h",
            "src/mpris_controller.cpp",
            "src/settings_model.h",
            "src/settings_model.cpp",
            "src/url_display.h",
            "src/url_display.cpp",
        ])
        .qrc(&qrc_path)
        .qt_module("Gui")
        .qt_module("Quick")
        .qt_module("WebEngineCore")
        .qt_module("WebEngineQuick")
        .qt_module("Network")
        .qt_module("DBus")
        .build()
        .export();
}

fn collect_browser_resources(root: &Path) -> Vec<PathBuf> {
    fn visit(path: &Path, resources: &mut Vec<PathBuf>) {
        let mut entries = fs::read_dir(path)
            .unwrap_or_else(|error| {
                panic!("read QML resource directory {}: {error}", path.display())
            })
            .map(|entry| entry.expect("enumerate QML resource").path())
            .collect::<Vec<_>>();
        entries.sort();
        for entry in entries {
            if entry.is_dir() {
                visit(&entry, resources);
            } else if matches!(
                entry.extension().and_then(|extension| extension.to_str()),
                Some("qml" | "js")
            ) {
                resources.push(entry);
            }
        }
    }

    let mut resources = Vec::new();
    visit(root, &mut resources);
    resources
}

fn collect_qml_files(root: &Path) -> Vec<PathBuf> {
    collect_browser_resources(root)
        .into_iter()
        .filter(|path| path.extension().is_some_and(|extension| extension == "qml"))
        .collect()
}

fn write_script_manifest(resources: &[PathBuf]) -> PathBuf {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set for build scripts"));
    let mut entries = Vec::with_capacity(resources.len());
    for path in resources {
        println!("cargo::rerun-if-changed={}", path.display());
        let source = fs::read(path).expect("read browser resource");
        entries.push(format!(
            "    {{\"path\":\"{}\",\"sha256\":\"{}\"}}",
            path.to_string_lossy().replace('\\', "/"),
            sha256_hex(&source)
        ));
    }
    let manifest = format!(
        "{{\n  \"schema\": 1,\n  \"bundle_version\": \"1\",\n  \"resources\": [\n{}\n  ]\n}}\n",
        entries.join(",\n")
    );
    let manifest_path = out_dir.join("browser-script-manifest.json");
    fs::write(&manifest_path, manifest).expect("write the generated browser script manifest");
    manifest_path
}

fn write_manifest_qrc(qrc_path: &Path, manifest_path: &Path, resources: &[PathBuf]) {
    let script_resources = resources
        .iter()
        .filter(|path| path.extension().is_some_and(|extension| extension == "js"))
        .map(|path| {
            let alias = path
                .strip_prefix("qml")
                .expect("browser resource is beneath qml")
                .to_string_lossy()
                .replace('\\', "/");
            let source = path
                .canonicalize()
                .expect("browser resource is readable before resource compilation");
            format!("    <file alias=\"{alias}\">{}</file>", source.display())
        })
        .collect::<Vec<_>>()
        .join("\n");
    let qrc = format!(
        "<RCC>\n  <qresource prefix=\"/qt/qml/io/github/ferricbrowser\">\n    <file alias=\"browser-script-manifest.json\">{}</file>\n{}\n  </qresource>\n</RCC>\n",
        manifest_path.display(),
        script_resources
    );
    fs::write(qrc_path, qrc).expect("write the generated browser script resource file");
    println!("cargo::rerun-if-changed={}", qrc_path.display());
}

#[allow(clippy::too_many_lines)]
fn sha256_hex(input: &[u8]) -> String {
    const INITIAL: [u32; 8] = [
        0x6a09_e667,
        0xbb67_ae85,
        0x3c6e_f372,
        0xa54f_f53a,
        0x510e_527f,
        0x9b05_688c,
        0x1f83_d9ab,
        0x5be0_cd19,
    ];
    const ROUND: [u32; 64] = [
        0x428a_2f98,
        0x7137_4491,
        0xb5c0_fbcf,
        0xe9b5_dba5,
        0x3956_c25b,
        0x59f1_11f1,
        0x923f_82a4,
        0xab1c_5ed5,
        0xd807_aa98,
        0x1283_5b01,
        0x2431_85be,
        0x550c_7dc3,
        0x72be_5d74,
        0x80de_b1fe,
        0x9bdc_06a7,
        0xc19b_f174,
        0xe49b_69c1,
        0xefbe_4786,
        0x0fc1_9dc6,
        0x240c_a1cc,
        0x2de9_2c6f,
        0x4a74_84aa,
        0x5cb0_a9dc,
        0x76f9_88da,
        0x983e_5152,
        0xa831_c66d,
        0xb003_27c8,
        0xbf59_7fc7,
        0xc6e0_0bf3,
        0xd5a7_9147,
        0x06ca_6351,
        0x1429_2967,
        0x27b7_0a85,
        0x2e1b_2138,
        0x4d2c_6dfc,
        0x5338_0d13,
        0x650a_7354,
        0x766a_0abb,
        0x81c2_c92e,
        0x9272_2c85,
        0xa2bf_e8a1,
        0xa81a_664b,
        0xc24b_8b70,
        0xc76c_51a3,
        0xd192_e819,
        0xd699_0624,
        0xf40e_3585,
        0x106a_a070,
        0x19a4_c116,
        0x1e37_6c08,
        0x2748_774c,
        0x34b0_bcb5,
        0x391c_0cb3,
        0x4ed8_aa4a,
        0x5b9c_ca4f,
        0x682e_6ff3,
        0x748f_82ee,
        0x78a5_636f,
        0x84c8_7814,
        0x8cc7_0208,
        0x90be_fffa,
        0xa450_6ceb,
        0xbef9_a3f7,
        0xc671_78f2,
    ];

    let bit_len = (input.len() as u64).wrapping_mul(8);
    let padded_len = (input.len() + 9).div_ceil(64) * 64;
    let mut padded = vec![0_u8; padded_len];
    padded[..input.len()].copy_from_slice(input);
    padded[input.len()] = 0x80;
    padded[padded_len - 8..].copy_from_slice(&bit_len.to_be_bytes());

    let mut state = INITIAL;
    for block in padded.chunks_exact(64) {
        let mut words = [0_u32; 64];
        for (index, bytes) in block.chunks_exact(4).take(16).enumerate() {
            words[index] = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        }
        for index in 16..64 {
            let value = words[index - 15].rotate_right(7)
                ^ words[index - 15].rotate_right(18)
                ^ (words[index - 15] >> 3);
            let next = words[index - 2].rotate_right(17)
                ^ words[index - 2].rotate_right(19)
                ^ (words[index - 2] >> 10);
            words[index] = words[index - 16]
                .wrapping_add(value)
                .wrapping_add(words[index - 7])
                .wrapping_add(next);
        }

        let mut working = state;
        for index in 0..64 {
            let choice = (working[4] & working[5]) ^ (!working[4] & working[6]);
            let majority =
                (working[0] & working[1]) ^ (working[0] & working[2]) ^ (working[1] & working[2]);
            let sigma_one = working[4].rotate_right(6)
                ^ working[4].rotate_right(11)
                ^ working[4].rotate_right(25);
            let sigma_zero = working[0].rotate_right(2)
                ^ working[0].rotate_right(13)
                ^ working[0].rotate_right(22);
            let temp_one = working[7]
                .wrapping_add(sigma_one)
                .wrapping_add(choice)
                .wrapping_add(ROUND[index])
                .wrapping_add(words[index]);
            let temp_two = sigma_zero.wrapping_add(majority);
            working[7] = working[6];
            working[6] = working[5];
            working[5] = working[4];
            working[4] = working[3].wrapping_add(temp_one);
            working[3] = working[2];
            working[2] = working[1];
            working[1] = working[0];
            working[0] = temp_one.wrapping_add(temp_two);
        }
        for index in 0..8 {
            state[index] = state[index].wrapping_add(working[index]);
        }
    }

    let mut output = String::with_capacity(64);
    for word in state {
        write!(output, "{word:08x}").expect("writing to a string cannot fail");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::sha256_hex;

    #[test]
    fn sha256_matches_standard_vectors() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
