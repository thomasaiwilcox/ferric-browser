//! Exact native-window correlation against the qualified public Lua interface.
use super::{HyprlandAdapter, HyprlandError, TriState, lua_string_literal};
use crate::page_window_native::NativeWindowIdentity;
use serde::Serialize;
use std::collections::HashSet;

pub(super) const QUALIFIED_COMMIT: &str = "efb50993780079460b0cbed1363e2166a2de1d9f";
const MAX_WINDOWS: usize = 64;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct BoundWindow {
    pub(crate) identity: NativeWindowIdentity,
    pub(crate) instance: String,
    pub(crate) stable_id: u64,
    pub(crate) address: String,
}

fn failure() -> HyprlandError {
    HyprlandError::Protocol("Native window ownership could not be verified".into())
}

fn parse_resolved(
    response: &str,
    requested: &[NativeWindowIdentity],
    instance: &str,
) -> Result<Vec<Option<BoundWindow>>, HyprlandError> {
    if response.len() > MAX_WINDOWS * 96 {
        return Err(failure());
    }
    let lines = response.trim().lines().collect::<Vec<_>>();
    if lines.len() != requested.len() {
        return Err(failure());
    }
    let mut ids = HashSet::new();
    let mut addresses = HashSet::new();
    lines
        .into_iter()
        .zip(requested)
        .enumerate()
        .map(|(index, (line, identity))| {
            let fields = line.split('|').collect::<Vec<_>>();
            if fields.first().and_then(|value| value.parse::<usize>().ok()) != Some(index + 1) {
                return Err(failure());
            }
            if matches!(fields.as_slice(), [_, "missing"]) {
                return Ok(None);
            }
            let [_, "ok", stable, address] = fields.as_slice() else {
                return Err(failure());
            };
            let stable_id = stable
                .parse::<u64>()
                .ok()
                .filter(|id| *id > 0)
                .ok_or_else(failure)?;
            let hexadecimal = address.strip_prefix("0x").ok_or_else(failure)?;
            let numeric_address = u64::from_str_radix(hexadecimal, 16)
                .ok()
                .filter(|address| *address != 0)
                .ok_or_else(failure)?;
            if hexadecimal.is_empty()
                || hexadecimal.len() > 16
                || !hexadecimal.bytes().all(|byte| byte.is_ascii_hexdigit())
                || !ids.insert(stable_id)
                || !addresses.insert(numeric_address)
            {
                return Err(failure());
            }
            Ok(Some(BoundWindow {
                identity: identity.clone(),
                instance: instance.into(),
                stable_id,
                address: (*address).into(),
            }))
        })
        .collect()
}

impl HyprlandAdapter {
    /// This method is read-only. Its owned scalar result may be used by a
    /// worker, but must be revalidated inside every subsequent Lua mutation.
    pub(crate) fn resolve_native_windows(
        &self,
        identities: &[NativeWindowIdentity],
    ) -> Result<Vec<Option<BoundWindow>>, HyprlandError> {
        if identities.is_empty() {
            return Ok(Vec::new());
        }
        if self.enabled == TriState::Off {
            return Err(HyprlandError::Disabled);
        }
        if identities.len() > MAX_WINDOWS {
            return Err(failure());
        }
        let mut tags = HashSet::new();
        for identity in identities {
            if NativeWindowIdentity::checked(&identity.tag, &identity.app_id, identity.pid).as_ref()
                != Some(identity)
                || !tags.insert(&identity.tag)
            {
                return Err(failure());
            }
        }
        let instance = self
            .request_socket
            .as_ref()
            .and_then(|path| path.parent())
            .and_then(|path| path.file_name())
            .and_then(|name| name.to_str())
            .filter(|name| {
                !name.is_empty()
                    && name.len() <= 128
                    && name
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
            })
            .ok_or_else(failure)?;
        let version = self.version()?;
        if version.commit.as_deref() != Some(QUALIFIED_COMMIT) {
            return Err(HyprlandError::Unavailable(
                "This compositor build has not qualified native page grouping".into(),
            ));
        }
        let requests = identities
            .iter()
            .map(|identity| {
                format!(
                    "{{tag={},app={},pid={}}}",
                    lua_string_literal(&identity.tag),
                    lua_string_literal(&identity.app_id),
                    identity.pid
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let expression = format!(
            "return (function() local all=hl.get_windows() local requested={{{requests}}} local out={{}} \
            for i,r in ipairs(requested) do local found=nil local count=0 \
            for _,w in ipairs(all) do if w.xdg_tag==r.tag then count=count+1 found=w end end \
            if count==0 then out[i]=tostring(i)..'|missing' \
            elseif count~=1 then return 'reject-ambiguous' \
            elseif found.pid~=r.pid or (found.class~=r.app and found.initial_class~=r.app) then return 'reject-identity' \
            else out[i]=tostring(i)..'|ok|'..tostring(found.stable_id)..'|'..tostring(found.address) end end \
            return table.concat(out,'\\n') end)()"
        );
        parse_resolved(
            &self.request(&format!("/repl {expression}"))?,
            identities,
            instance,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        io::{Read, Write},
        os::unix::net::UnixListener,
        thread,
    };
    fn identity() -> NativeWindowIdentity {
        NativeWindowIdentity::checked(
            &format!("ferric-page-{}", crate::Uuid::new_v4()),
            "application_tests",
            7,
        )
        .unwrap()
    }
    #[test]
    fn bindings_reject_reordered_duplicated_or_malformed_native_handles() {
        let identities = [identity(), identity()];
        let result = parse_resolved("1|ok|41|0xabc\n2|missing", &identities, "owned").unwrap();
        assert_eq!(result[0].as_ref().unwrap().identity, identities[0]);
        assert_eq!(result[0].as_ref().unwrap().stable_id, 41);
        assert!(result[1].is_none());
        for malformed in [
            "2|ok|41|0xabc\n1|missing",
            "1|ok|41|0xabc\n2|ok|41|0xdef",
            "1|ok|41|0xabc\n2|ok|42|0xabc",
            "1|ok|41|0xabc\n2|ok|42|0x0ABC",
            "1|ok|0|0xabc\n2|missing",
            "1|ok|41|0x0\n2|missing",
            "1|ok|41|0xnothex\n2|missing",
            "reject-ambiguous",
            "1|missing\n2|missing\n3|missing",
        ] {
            assert!(parse_resolved(malformed, &identities, "owned").is_err());
        }
    }
    #[test]
    fn native_resolution_uses_one_bounded_read_and_does_not_require_workspace_routing() {
        let root = std::env::temp_dir().join(format!("ferric-native-{}", crate::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let socket = root.join(".socket.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        let identity = identity();
        let tag = identity.tag.clone();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut command = String::new();
            stream.read_to_string(&mut command).unwrap();
            assert_eq!(command, "j/version");
            stream
                .write_all(
                    format!("{{\"tag\":\"v0.56.2\",\"commit\":\"{QUALIFIED_COMMIT}\"}}").as_bytes(),
                )
                .unwrap();
            drop(stream);
            let (mut stream, _) = listener.accept().unwrap();
            command.clear();
            stream.read_to_string(&mut command).unwrap();
            assert!(command.starts_with("/repl return "));
            assert!(command.contains(&tag));
            assert!(command.contains("count~=1") && command.contains("found.pid~=r.pid"));
            assert!(!command.contains("hl.dispatch"));
            stream.write_all(b"1|ok|41|0xabc").unwrap();
        });
        let adapter = HyprlandAdapter::with_paths(TriState::On, false, socket, root.join("events"));
        let result = adapter.resolve_native_windows(&[identity]).unwrap();
        assert_eq!(result[0].as_ref().unwrap().address, "0xabc");
        server.join().unwrap();
        fs::remove_dir_all(root).unwrap();
    }
}
