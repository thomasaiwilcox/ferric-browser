//! Exact native-window correlation against the qualified public Lua interface.
use super::{HyprlandAdapter, HyprlandError, TriState, lua_string_literal};
use crate::Uuid;
use crate::page_window_native::NativeWindowIdentity;
use serde::Serialize;
use std::collections::HashSet;

pub(super) const QUALIFIED_COMMIT: &str = "efb50993780079460b0cbed1363e2166a2de1d9f";
const MAX_WINDOWS: usize = 64;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum GroupOutcome {
    Changed,
    Unchanged,
    Deferred,
    Rejected,
    Uncertain,
}

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

pub(crate) struct PreparedGroupWrite {
    request: super::unix_deadline::PreparedRequest,
    command: String,
    expected: ManagedRequest,
    bindings: Vec<BoundWindow>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ManagedCapability {
    pub(crate) key: Uuid,
    pub(crate) scope: Uuid,
    pub(crate) instance: String,
}

pub(crate) struct ManagedRole {
    pub(crate) binding: BoundWindow,
    pub(crate) scope: Uuid,
    pub(crate) auto: bool,
    pub(crate) ready: bool,
}

#[derive(Clone)]
pub(crate) struct ManagedRequest {
    pub(crate) process: Uuid,
    pub(crate) epoch: Uuid,
    pub(crate) scope: Uuid,
    pub(crate) fresh: Uuid,
    pub(crate) capabilities: Vec<ManagedCapability>,
    pub(crate) allow_create: bool,
}

impl ManagedRequest {
    pub(crate) fn new(
        process: Uuid,
        epoch: Uuid,
        scope: Uuid,
        capabilities: Vec<ManagedCapability>,
    ) -> Self {
        Self {
            process,
            epoch,
            scope,
            fresh: Uuid::new_v4(),
            allow_create: capabilities.len() < 128,
            capabilities,
        }
    }

    pub(crate) fn restrict_to_instance(&mut self, instance: &str) {
        // Capacity belongs to the process ledger, before instance filtering.
        self.capabilities.retain(|cap| cap.instance == instance);
    }
}

#[derive(Debug)]
pub(crate) struct ManagedReceipt {
    pub(crate) capability: ManagedCapability,
    pub(crate) members: Vec<NativeWindowIdentity>,
}

#[derive(Debug)]
pub(crate) struct GroupReply {
    pub(crate) outcome: GroupOutcome,
    pub(crate) receipt: Option<ManagedReceipt>,
}

impl From<GroupOutcome> for GroupReply {
    fn from(outcome: GroupOutcome) -> Self {
        Self {
            outcome,
            receipt: None,
        }
    }
}

pub(crate) struct PendingGroupReply {
    response: super::unix_deadline::PendingResponse,
    expected: ManagedRequest,
    bindings: Vec<BoundWindow>,
}

impl PreparedGroupWrite {
    pub(crate) fn deadline(&self) -> std::time::Instant {
        self.request.deadline()
    }
    /// GUI-only, immediately after final admission. No waits or callbacks.
    pub(crate) fn dispatch(self) -> Result<PendingGroupReply, GroupOutcome> {
        self.request
            .dispatch(self.command.as_bytes())
            .map(|response| PendingGroupReply {
                response,
                expected: self.expected,
                bindings: self.bindings,
            })
            .map_err(|error| match error {
                super::unix_deadline::DispatchError::NotSent(_) => GroupOutcome::Deferred,
                super::unix_deadline::DispatchError::PossiblySent(_) => GroupOutcome::Uncertain,
            })
    }
}

fn parse_group_reply(
    response: &str,
    expected: &ManagedRequest,
    bindings: &[BoundWindow],
) -> GroupReply {
    let response = response.trim();
    if response.len() <= 64
        && response
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte == b'-')
    {
        if response.starts_with("defer-") {
            return GroupOutcome::Deferred.into();
        }
        if response.starts_with("reject-") {
            return GroupOutcome::Rejected.into();
        }
    }
    let fields = response.split('|').collect::<Vec<_>>();
    let [kind, key, scope, ids] = fields.as_slice() else {
        return GroupOutcome::Uncertain.into();
    };
    let Some(key) = Uuid::parse_str(key)
        .ok()
        .filter(|id| !id.is_nil() && id.to_string() == *key)
    else {
        return GroupOutcome::Uncertain.into();
    };
    let Some(scope) = Uuid::parse_str(scope)
        .ok()
        .filter(|id| !id.is_nil() && id.to_string() == *scope)
    else {
        return GroupOutcome::Uncertain.into();
    };
    let Some(instance) = bindings.first().map(|binding| binding.instance.clone()) else {
        return GroupOutcome::Uncertain.into();
    };
    let capability = ManagedCapability {
        key,
        scope,
        instance,
    };
    let created = *kind == "created"
        && expected.allow_create
        && key == expected.fresh
        && scope == expected.scope
        && expected.capabilities.len() < 128;
    let existing = matches!(*kind, "added" | "unchanged" | "retired" | "forgotten")
        && expected.capabilities.contains(&capability);
    if !created && !existing {
        return GroupOutcome::Uncertain.into();
    }
    let mut seen = HashSet::new();
    let members = if ids.is_empty() {
        Some(Vec::new())
    } else {
        ids.split(',')
            .map(|id| {
                id.parse::<u64>().ok().filter(|value| {
                    *value > 0
                        && value.to_string() == id
                        && bindings.iter().any(|binding| binding.stable_id == *value)
                        && seen.insert(*value)
                })
            })
            .collect::<Option<Vec<_>>>()
    };
    let Some(members) = members.filter(|members| {
        members.len() <= MAX_WINDOWS
            && (!members.is_empty() || matches!(*kind, "retired" | "forgotten"))
            && (*kind != "forgotten" || members.is_empty())
            && (!created || members.len() == 1)
    }) else {
        return GroupOutcome::Uncertain.into();
    };
    GroupReply {
        outcome: if matches!(*kind, "forgotten" | "retired") {
            GroupOutcome::Deferred
        } else if *kind == "unchanged" {
            GroupOutcome::Unchanged
        } else {
            GroupOutcome::Changed
        },
        receipt: Some(ManagedReceipt {
            capability,
            members: members
                .into_iter()
                .filter_map(|id| {
                    bindings
                        .iter()
                        .find(|binding| binding.stable_id == id)
                        .map(|binding| binding.identity.clone())
                })
                .collect(),
        }),
    }
}

impl PendingGroupReply {
    pub(crate) fn finish(self) -> GroupReply {
        match self
            .response
            .finish(2048)
            .ok()
            .and_then(|bytes| String::from_utf8(bytes).ok())
        {
            Some(response) => parse_group_reply(&response, &self.expected, &self.bindings),
            None => GroupOutcome::Uncertain.into(),
        }
    }
}

impl HyprlandAdapter {
    /// Prepare on the worker; only the final one-shot send runs on the GUI.
    /// Capabilities are immutable birth-scope receipts from actual creation.
    pub(crate) fn prepare_native_group(
        &self,
        roles: &[ManagedRole],
        expected: ManagedRequest,
    ) -> Result<PreparedGroupWrite, HyprlandError> {
        if self.enabled == TriState::Off {
            return Err(HyprlandError::Disabled);
        }
        if roles.is_empty()
            || roles.len() > MAX_WINDOWS
            || expected.capabilities.len() > 128
            || [
                expected.process,
                expected.epoch,
                expected.scope,
                expected.fresh,
            ]
            .iter()
            .any(Uuid::is_nil)
            || roles.iter().any(|role| role.scope.is_nil())
        {
            return Err(failure());
        }
        let instance = self
            .request_socket
            .as_ref()
            .and_then(|p| p.parent())
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str())
            .ok_or_else(failure)?;
        let bindings = roles
            .iter()
            .map(|role| role.binding.clone())
            .collect::<Vec<_>>();
        let first = &bindings[0].identity;
        if bindings.iter().any(|binding| {
            binding.instance != instance
                || binding.identity.pid != first.pid
                || binding.identity.app_id != first.app_id
        }) || expected
            .capabilities
            .iter()
            .any(|cap| cap.instance != instance || cap.key.is_nil() || cap.scope.is_nil())
        {
            return Err(failure());
        }
        let identities = bindings
            .iter()
            .map(|b| b.identity.clone())
            .collect::<Vec<_>>();
        let handles = bindings
            .iter()
            .enumerate()
            .map(|(i, b)| format!("{}|ok|{}|{}", i + 1, b.stable_id, b.address))
            .collect::<Vec<_>>()
            .join("\n");
        parse_resolved(&handles, &identities, instance)?;
        let requests = roles
            .iter()
            .map(|role| {
                let binding = &role.binding;
                format!(
                    "{{tag={},app={},pid={},stable={},address={},scope={},auto={},ready={}}}",
                    lua_string_literal(&binding.identity.tag),
                    lua_string_literal(&binding.identity.app_id),
                    binding.identity.pid,
                    lua_string_literal(&binding.stable_id.to_string()),
                    lua_string_literal(&binding.address),
                    lua_string_literal(&role.scope.to_string()),
                    role.auto,
                    role.ready
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let caps = expected
            .capabilities
            .iter()
            .map(|cap| {
                format!(
                    "{{key={},scope={}}}",
                    lua_string_literal(&cap.key.to_string()),
                    lua_string_literal(&cap.scope.to_string())
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let transaction = include_str!("managed_groups.lua");
        let options = format!(
            "{{process={},epoch={},scope={},fresh={},pid={},app={},allow_create={},caps={{{caps}}}}}",
            lua_string_literal(&expected.process.to_string()),
            lua_string_literal(&expected.epoch.to_string()),
            lua_string_literal(&expected.scope.to_string()),
            lua_string_literal(&expected.fresh.to_string()),
            first.pid,
            lua_string_literal(&first.app_id),
            expected.allow_create
        );
        let command = format!(
            "/repl return (function() local apply=(function()\n{transaction}\nend)() \
            local ok,result=pcall(apply,{{{requests}}},{options}) \
            if not ok then return 'uncertain-lua-error' end return result end)()"
        );
        let path = self.request_socket.as_deref().ok_or_else(failure)?;
        let request = super::unix_deadline::PreparedRequest::connect(path, super::IO_TIMEOUT)
            .map_err(|error| HyprlandError::Io(error.to_string()))?;
        Ok(PreparedGroupWrite {
            request,
            command,
            expected,
            bindings,
        })
    }

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
    fn managed_receipts_bind_birth_scope_capability_instance_and_exact_members() {
        let binding = BoundWindow {
            identity: identity(),
            instance: "owned".into(),
            stable_id: 41,
            address: "0xabc".into(),
        };
        let mut expected =
            ManagedRequest::new(Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4(), Vec::new());
        let created = format!("created|{}|{}|41", expected.fresh, expected.scope);
        let reply = parse_group_reply(&created, &expected, std::slice::from_ref(&binding));
        assert_eq!(reply.outcome, GroupOutcome::Changed);
        let receipt = reply.receipt.unwrap();
        assert_eq!(
            receipt.members.as_slice(),
            std::slice::from_ref(&binding.identity)
        );
        expected.capabilities.push(receipt.capability.clone());
        let key = expected.fresh;
        let scope = expected.scope;
        let retired = parse_group_reply(
            &format!("retired|{key}|{scope}|"),
            &expected,
            std::slice::from_ref(&binding),
        );
        assert!(retired.receipt.unwrap().members.is_empty());
        for invalid in [
            format!("created|{key}|{}|41", Uuid::new_v4()),
            format!("added|{}|{scope}|41", Uuid::new_v4()),
            format!("added|{key}|{scope}|41,41"),
            format!("added|{key}|{scope}|42"),
            format!("added|{key}|{scope}|041"),
            format!("unchanged|{key}|{scope}|"),
            format!("retired|{key}|{scope}|41|extra"),
            "created".into(),
            "unknown".into(),
        ] {
            assert_eq!(
                parse_group_reply(&invalid, &expected, std::slice::from_ref(&binding)).outcome,
                GroupOutcome::Uncertain
            );
        }
        let caps = (0..128)
            .map(|i| ManagedCapability {
                key: Uuid::new_v4(),
                scope,
                instance: if i % 2 == 0 {
                    "owned".into()
                } else {
                    "old-instance".into()
                },
            })
            .collect();
        let mut full = ManagedRequest::new(expected.process, expected.epoch, scope, caps);
        full.restrict_to_instance("owned");
        assert_eq!(full.capabilities.len(), 64);
        assert!(!full.allow_create);
        let impossible = format!("created|{}|{}|41", full.fresh, full.scope);
        assert_eq!(
            parse_group_reply(&impossible, &full, std::slice::from_ref(&binding)).outcome,
            GroupOutcome::Uncertain
        );
        let forgotten = parse_group_reply(
            &format!("forgotten|{key}|{scope}|"),
            &expected,
            std::slice::from_ref(&binding),
        );
        assert_eq!(forgotten.outcome, GroupOutcome::Deferred);
        assert!(forgotten.receipt.unwrap().members.is_empty());
        expected.capabilities[0].instance = "replacement".into();
        assert_eq!(
            parse_group_reply(&format!("added|{key}|{scope}|41"), &expected, &[binding]).outcome,
            GroupOutcome::Uncertain
        );
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
