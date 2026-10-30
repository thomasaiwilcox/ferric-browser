-- One operation over fresh, GUI-admitted native/Core roles. The fixed global
-- root is bounded across process restarts. Stored userdata retain only weak
-- native references; Lua must retain the wrappers strongly and test liveness
-- before equality (two expired HL.Group wrappers otherwise compare equal).
return function(requested, options)
    local ROOT, MAX_GROUPS, MAX_MEMBERS = '__ferric_managed_page_groups', 128, 64
    local function token(value)
        if type(value) ~= 'string' or #value ~= 36
            or value == '00000000-0000-0000-0000-000000000000' then return false end
        local a,b,c,d,e = value:match('^([a-f0-9]+)%-([a-f0-9]+)%-([a-f0-9]+)%-([a-f0-9]+)%-([a-f0-9]+)$')
        return a and #a == 8 and #b == 4 and #c == 4 and #d == 4 and #e == 12
    end
    local function app(value)
        return type(value) == 'string' and #value >= 1 and #value <= 256
            and value:match('^[a-zA-Z0-9_.-]+$')
    end
    local function pid(value) return type(value) == 'number' and value > 0 and value % 1 == 0 end
    local function live(group)
        return type(group) == 'userdata' and group.size ~= nil
            and group.size >= 1 and group.size <= MAX_MEMBERS and group.current ~= nil
    end
    local function same(left, right) return live(left) and live(right) and left == right end
    if type(options) ~= 'table' or not token(options.process) or not token(options.epoch)
        or not token(options.scope) or not token(options.fresh) or not pid(options.pid)
        or not app(options.app) or type(options.allow_create) ~= 'boolean' or type(options.caps) ~= 'table' or #options.caps > MAX_GROUPS
        or #requested < 1 or #requested > MAX_MEMBERS then return 'reject-request' end
    local allowed = {}
    for _, cap in ipairs(options.caps) do
        if not token(cap.key) or not token(cap.scope) or allowed[cap.key] then return 'reject-capabilities' end
        allowed[cap.key] = cap.scope
    end
    local root = rawget(_G, ROOT)
    if root == nil then
        root = {owner='ferric-native-page-groups',version=1,entries={}}
        rawset(_G, ROOT, root)
    elseif type(root) ~= 'table' or root.owner ~= 'ferric-native-page-groups'
        or root.version ~= 1 or type(root.entries) ~= 'table' then return 'reject-registry-owner' end
    local count = 0
    for key, entry in pairs(root.entries) do
        count = count + 1
        if count > MAX_GROUPS or not token(key) or type(entry) ~= 'table'
            or not token(entry.process) or not token(entry.epoch) or not token(entry.scope)
            or not pid(entry.pid) or not app(entry.app) or type(entry.group) ~= 'userdata'
            or type(entry.seeds) ~= 'table' or #entry.seeds < 1 or #entry.seeds > MAX_MEMBERS then
            return 'reject-registry-bound'
        end
        for _, seed in ipairs(entry.seeds) do
            if type(seed) ~= 'userdata' then return 'reject-registry-bound' end
        end
    end
    -- Sweep globally, including crashed process namespaces, before allocation.
    for key, entry in pairs(root.entries) do
        local surviving = false
        if live(entry.group) then
            for _, seed in ipairs(entry.seeds) do
                if seed.pid == entry.pid and same(seed.group, entry.group) then surviving = true break end
            end
        end
        if not surviving then root.entries[key] = nil count = count - 1 end
    end
    -- Losing runtime state or the actual weak group retires only the supplied
    -- receipt. It grants no replacement capability and performs no native op.
    for _, cap in ipairs(options.caps) do
        if root.entries[cap.key] == nil then return 'forgotten|'..cap.key..'|'..cap.scope..'|' end
    end
    local function capability(group)
        if not live(group) then return nil, nil end
        for key, scope in pairs(allowed) do
            local entry = root.entries[key]
            if entry and entry.process == options.process and entry.epoch == options.epoch
                and entry.scope == scope and entry.pid == options.pid and entry.app == options.app
                and same(entry.group, group) then return entry, key end
        end
        return nil, nil
    end
    local all, windows, admitted, tags, addresses, orders = hl.get_windows(), {}, {}, {}, {}, {}
    for i, request in ipairs(requested) do
        if tags[request.tag] or addresses[request.address] or orders[request.order] or not token(request.scope)
            or type(request.auto) ~= 'boolean' or type(request.ready) ~= 'boolean'
            or type(request.order) ~= 'number' or request.order % 1 ~= 0 or request.order < 1 or request.order > MAX_MEMBERS
            or request.pid ~= options.pid or request.app ~= options.app then return 'reject-request' end
        tags[request.tag], addresses[request.address] = true, true
        orders[request.order] = true
        local found, matches = nil, 0
        for _, window in ipairs(all) do
            if window.xdg_tag == request.tag then found, matches = window, matches + 1 end
        end
        if matches ~= 1 or tostring(found.stable_id) ~= request.stable
            or tostring(found.address) ~= request.address or found.pid ~= request.pid
            or (found.class ~= request.app and found.initial_class ~= request.app)
            or admitted[found.stable_id] then return 'reject-identity' end
        windows[i], admitted[found.stable_id] = found, {window=found,role=request}
    end
    local function checked(group)
        if not live(group) then return nil, 'reject-membership' end
        if group.locked or group.denied then return nil, 'reject-locked' end
        local members, seen = group.members, {}
        if #members ~= group.size or #members > MAX_MEMBERS then return nil, 'reject-membership' end
        for _, member in ipairs(members) do
            local owner = admitted[member.stable_id]
            if not owner or owner.window ~= member or seen[member.stable_id] then return nil, 'reject-membership' end
            if not owner.role.ready then return nil, 'defer-presentation' end
            seen[member.stable_id] = true
        end
        if not seen[group.current.stable_id] then return nil, 'reject-membership' end
        return members, nil
    end
    local function receipt(kind, key, entry)
        local ids = {}
        if entry then
            for _, member in ipairs(entry.group.members) do ids[#ids+1] = tostring(member.stable_id) end
            if #ids ~= entry.group.size or #ids > MAX_MEMBERS then return 'uncertain-receipt' end
        end
        local scope = entry and entry.scope or allowed[key]
        return kind..'|'..key..'|'..scope..'|'..table.concat(ids, ',')
    end
    local active = hl.get_active_window()
    -- Retirement is the sole cross-scope / Auto-off operation. It requires a
    -- previously saved weak capability and every affected actual GUI role.
    if active and active.group then
        local entry, key = capability(active.group)
        local owner = admitted[active.stable_id]
        if entry and owner and (owner.role.scope ~= entry.scope or not owner.role.auto) then
            local group = entry.group
            local members, failure = checked(group)
            if failure then return failure end
            if not same(active.group, group) or hl.get_active_window() ~= active then return 'defer-active' end
            group:remove(active)
            if active.group or hl.get_active_window() ~= active then return 'uncertain-retirement' end
            local seeds = {}
            if live(group) then
                local remaining, remaining_failure = checked(group)
                if remaining_failure or #remaining ~= #members - 1 then return 'uncertain-retirement-members' end
                -- This is the same proven live native group, and every
                -- remaining member just passed fresh actual ownership checks.
                -- Retain removal authority even if the last original seed
                -- departed first; its immutable birth scope is unchanged.
                seeds = remaining
            elseif #members ~= 1 then return 'uncertain-retirement-group' end
            if #seeds == 0 then root.entries[key] = nil return receipt('retired', key, nil) end
            entry.seeds = seeds
            return receipt('retired', key, entry)
        end
    end
    local candidates, existing, entry, key = {}, nil, nil, nil
    for _, window in ipairs(windows) do
        local role = admitted[window.stable_id].role
        if role.scope == options.scope and role.auto then
            if not role.ready then return 'defer-presentation' end
            candidates[#candidates+1] = window
            if window.group then
                local members, failure = checked(window.group)
                if failure then return failure end
                local candidate_entry, candidate_key = capability(window.group)
                if not candidate_entry or candidate_entry.scope ~= options.scope then return 'reject-managed-capability' end
                for _, member in ipairs(members) do
                    local member_role = admitted[member.stable_id].role
                    if member_role.scope ~= options.scope or not member_role.auto then return 'defer-retirement' end
                end
                if existing and not same(existing, window.group) then return 'defer-separate-groups' end
                existing, entry, key = window.group, candidate_entry, candidate_key
            end
        end
    end
    if #candidates < 2 then return 'defer-pages' end
    if existing then
        local target = nil
        for _, window in ipairs(candidates) do
            if not window.group then
                if window == active then return 'defer-active' end
                target = window break
            end
        end
        if not target then
            local desired = existing.members
            -- Same proven live group; every member's current role has just
            -- passed same-birth-scope / Auto admission, including on-map peers.
            entry.seeds = desired
            table.sort(desired, function(left,right)
                return admitted[left.stable_id].role.order < admitted[right.stable_id].role.order
            end)
            local function ordered()
                for index, member in ipairs(existing.members) do if member ~= desired[index] then return false end end
                return true
            end
            if ordered() then return receipt('unchanged', key, entry) end
            -- Public move_window operates on the globally active group's
            -- current member. Never activate another member just to sort.
            if not active or not same(active.group, existing) or existing.current ~= active then return receipt('ordering', key, entry) end
            local desired_index = nil
            for index, member in ipairs(desired) do if member == active then desired_index = index break end end
            local before = existing.current_index
            if before == desired_index then return receipt('ordering', key, entry) end
            local forward = before < desired_index
            for _ = 1, math.abs(before - desired_index) do
                local previous = existing.current_index
                hl.dispatch(hl.dsp.group.move_window({forward=forward}))
                local members, failure = checked(existing)
                if failure or hl.get_active_window() ~= active or existing.current ~= active
                    or existing.current_index ~= previous + (forward and 1 or -1) then return 'uncertain-order' end
                entry.seeds = members
            end
            return receipt(ordered() and 'ordered' or 'ordering', key, entry)
        end
        if existing.size >= MAX_MEMBERS then return 'reject-membership' end
        if active and same(active.group, existing) then return 'defer-active' end
        local before_current, before_size = existing.current, existing.size
        local insert_at = 1
        for _, member in ipairs(existing.members) do
            if admitted[member.stable_id].role.order < admitted[target.stable_id].role.order then insert_at = insert_at + 1 end
        end
        existing:add(target, insert_at)
        local restore_index = nil
        for index, member in ipairs(existing.members) do if member == before_current then restore_index = index break end end
        if not restore_index then return 'uncertain-add-current' end
        hl.dispatch(hl.dsp.group.active({window=before_current,index=restore_index}))
        local members, failure = checked(existing)
        if failure or not same(target.group, existing) or existing.size ~= before_size + 1
            or existing.current ~= before_current or hl.get_active_window() ~= active then return 'uncertain-add' end
        entry.seeds = members
        return receipt('added', key, entry)
    end
    if not active or not admitted[active.stable_id] then return 'defer-active' end
    local role = admitted[active.stable_id].role
    if role.scope ~= options.scope or not role.auto or not role.ready or active.group then return 'defer-active' end
    if not options.allow_create or count >= MAX_GROUPS or #options.caps >= MAX_GROUPS or root.entries[options.fresh] then return 'reject-registry-allocation' end
    -- Capability minting encloses this exact active-window creation; arbitrary
    -- pre-existing user groups can never be adopted by membership inspection.
    hl.dispatch(hl.dsp.group.toggle({window=active}))
    local group = active.group
    if not live(group) or group.size ~= 1 or group.current ~= active or group.locked or group.denied
        or hl.get_active_window() ~= active then return 'uncertain-create' end
    local created = {process=options.process,epoch=options.epoch,scope=options.scope,
        pid=options.pid,app=options.app,group=group,seeds={active}}
    root.entries[options.fresh] = created
    return receipt('created', options.fresh, created)
end
