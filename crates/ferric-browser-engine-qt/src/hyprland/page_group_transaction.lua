-- Called only with an internally admitted, same-scope logical group. Values
-- are bound native identities, not titles, selectors or renderer envelopes.
return function(requested, action, source_index, target_index)
    if #requested < 1 or #requested > 64 then return 'reject-request' end
    local all = hl.get_windows()
    local windows, admitted, tags, addresses = {}, {}, {}, {}
    for i, r in ipairs(requested) do
        if tags[r.tag] or addresses[r.address] then return 'reject-request' end
        tags[r.tag], addresses[r.address] = true, true
        local found, count = nil, 0
        for _, w in ipairs(all) do
            if w.xdg_tag == r.tag then found, count = w, count + 1 end
        end
        if count ~= 1 or tostring(found.stable_id) ~= r.stable
            or tostring(found.address) ~= r.address or found.pid ~= r.pid
            or (found.class ~= r.app and found.initial_class ~= r.app)
            or admitted[found.stable_id] then return 'reject-identity' end
        windows[i], admitted[found.stable_id] = found, found
    end
    local source, target = windows[source_index], windows[target_index]
    if not source then return 'reject-request' end
    local function checked_group(window)
        local group = window.group
        if not group then return nil, nil end
        if group.locked or group.denied then return nil, 'reject-locked' end
        local members, seen = group.members, {}
        if #members ~= group.size or #members < 1 or #members > 64
            or not group.current then return nil, 'reject-membership' end
        for _, member in ipairs(members) do
            if not admitted[member.stable_id] or seen[member.stable_id]
                or admitted[member.stable_id] ~= member then
                return nil, 'reject-membership'
            end
            seen[member.stable_id] = true
        end
        if not seen[window.stable_id] or not seen[group.current.stable_id]
            then return nil, 'reject-membership' end
        return group, nil
    end
    local active = hl.get_active_window()
    if action == 'reconcile' then
        if #windows < 2 then return 'unchanged' end
        local seed, existing = nil, nil
        for _, window in ipairs(windows) do
            local candidate, failure = checked_group(window)
            if failure then return failure end
            if candidate then
                if existing and existing ~= candidate then return 'defer-separate-groups' end
                existing, seed = candidate, window
            end
        end
        if existing then
            target = nil
            for _, window in ipairs(windows) do
                if not window.group and window ~= active then target = window break end
            end
            if not target then
                for _, window in ipairs(windows) do
                    if not window.group then return 'defer-active' end
                end
                return 'unchanged'
            end
            source, action = seed, 'add'
        else
            source = nil
            for _, window in ipairs(windows) do if window == active then source = window break end end
            if not source then return 'defer-active' end
            action = 'create'
        end
    end
    local group, err = checked_group(source)
    if err then return err end
    local function active_id() local w = hl.get_active_window() return w and w.stable_id end
    local before_active = active and active.stable_id
    if action == 'create' then
        if group then return 'unchanged' end
        -- Never create a group by briefly focusing a background window.
        if not active or active ~= source then return 'defer-active' end
        hl.dispatch(hl.dsp.group.toggle({window=source}))
        local created, failure = checked_group(source)
        if failure then return 'uncertain-created-group' end
        if not created or created.size ~= 1 or active_id() ~= before_active
            then return 'uncertain-create-postcondition' end
        return 'created'
    elseif action == 'add' then
        if not target or target == source then return 'reject-request' end
        if not group then return 'defer-group' end
        if target.group then
            if target.group == group then return 'unchanged' end
            return 'reject-target-group'
        end
        if group.size >= 64 then return 'reject-membership' end
        -- Adding hides the current member temporarily. On the qualified build
        -- an active group can emit delayed focus changes even after restoring
        -- its current member. Only add while focus is outside the group.
        if active == target or (active and active.group == group) then return 'defer-active' end
        local before_current, before_index, before_size = group.current, group.current_index, group.size
        -- Supply an explicit append index; desktop insert-after-current settings
        -- must not reorder the app's existing members.
        group:add(target, before_size + 1)
        hl.dispatch(hl.dsp.group.active({window=source, index=before_index}))
        local after, failure = checked_group(source)
        if failure then return 'uncertain-added-group' end
        if not after or after ~= target.group or after.size ~= before_size + 1
            or after.current ~= before_current or active_id() ~= before_active
            then return 'uncertain-add-postcondition' end
        return 'added'
    elseif action == 'remove' then
        if not group then return 'unchanged' end
        local before_current = group.current
        -- CGroup::remove assigns a new layout space and can focus even a
        -- noncurrent member. Only remove the exact window already active;
        -- restoring focus afterwards cannot undo a transient focus event.
        if active ~= source then return 'defer-active' end
        group:remove(source)
        if source.group then return 'uncertain-remove-membership' end
        if before_current ~= source then
            local remaining = before_current.group
            if remaining then
                for index, member in ipairs(remaining.members) do
                    if member == before_current then
                        hl.dispatch(hl.dsp.group.active({window=before_current, index=index}))
                        break
                    end
                end
            end
            local remaining, failure = checked_group(before_current)
            if failure then return 'uncertain-remaining-group' end
            if not remaining or remaining.current ~= before_current
                then return 'uncertain-remove-current' end
        end
        if active_id() ~= before_active then return 'uncertain-remove-focus' end
        return 'removed'
    elseif action == 'focus' then
        hl.dispatch(hl.dsp.focus({window=source}))
        if active_id() ~= source.stable_id then return 'uncertain-focus-postcondition' end
        return 'focused'
    end
    return 'reject-request'
end
