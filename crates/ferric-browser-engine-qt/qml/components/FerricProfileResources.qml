import QtQuick
import QtWebEngine
import io.github.ferricbrowser 1.0

// Created by the application factory. Native consumers, policy projection and
// signal handlers outlive the window that first opened this profile.
QtObject {
    id: resources
    required property var rootWindow
    required property string profileName
    required property string storageBase
    required property bool transientProfile
    property var existingProfile: null
    property var existingPrototype: null
    property var existingInterceptor: null
    property var profile: null
    property var profilePrototype: null
    property var managedInterceptor: null
    readonly property var interceptor: existingInterceptor || managedInterceptor
    readonly property alias lifetime: lifetime
    readonly property var policyUi: lifetime.policyUi
    property bool usable: false
    property var policySnapshot: ({enabled:false,blockedHosts:[],exceptionHosts:[],blockedRuleHosts:[],blockedRuleListIds:[],exceptionRuleHosts:[],exceptionRuleListIds:[],bypassSites:[],securityDenyHosts:[],adblockEngineHandle:0})

    property FerricProfileLifetime nativeLifetime: FerricProfileLifetime {
        id: lifetime
        onViewRetiring: function(view) { resources.retireView(view) }
        onDrained: resources.rootWindow.profileResourceRegistry.release(resources)
    }
    property Component profileFactory: Component {
        WebEngineProfilePrototype {
            storageName: resources.transientProfile ? "" : "ferric-browser-" + resources.profileName
            persistentStoragePath: resources.transientProfile || resources.storageBase.length === 0
                                   ? "" : resources.storageBase + "/webengine/" + resources.profileName
            cachePath: resources.transientProfile || resources.storageBase.length === 0
                       ? "" : resources.storageBase + "/webengine-cache/" + resources.profileName
            persistentPermissionsPolicy: WebEngineProfile.AskEveryTime
        }
    }
    property Component interceptorFactory: Component {
      RequestInterceptor {
        blockedHosts: resources.policySnapshot.blockedHosts
        exceptionHosts: resources.policySnapshot.exceptionHosts
        blockedRuleHosts: resources.policySnapshot.blockedRuleHosts
        blockedRuleListIds: resources.policySnapshot.blockedRuleListIds
        exceptionRuleHosts: resources.policySnapshot.exceptionRuleHosts
        exceptionRuleListIds: resources.policySnapshot.exceptionRuleListIds
        bypassSites: resources.policySnapshot.bypassSites
        securityDenyHosts: resources.policySnapshot.securityDenyHosts
        enabled: resources.policySnapshot.enabled
        adblockEngineHandle: resources.policySnapshot.adblockEngineHandle
      }
    }

    // Keep the last valid snapshot while consumers retire. Releasing a UI
    // must never clear the interceptor or replace blocking policy with defaults.
    function refreshPolicy() {
        var ui=policyUi
        if (!ui || ui.profile_bootstrap_pending || ui.blocking_policy_ready===false
                || existingInterceptor) return
        policySnapshot=({
            enabled: ui.blocking_enabled || ui.blocking_security_deny_hosts.length > 0,
            blockedHosts: Array.prototype.slice.call(ui.blocking_hosts),
            exceptionHosts: Array.prototype.slice.call(ui.blocking_exceptions),
            blockedRuleHosts: Array.prototype.slice.call(ui.blocking_rule_hosts),
            blockedRuleListIds: Array.prototype.slice.call(ui.blocking_rule_list_ids),
            exceptionRuleHosts: Array.prototype.slice.call(ui.blocking_exception_rule_hosts),
            exceptionRuleListIds: Array.prototype.slice.call(ui.blocking_exception_rule_list_ids),
            bypassSites: Array.prototype.slice.call(ui.blocking_bypass_sites),
            securityDenyHosts: Array.prototype.slice.call(ui.blocking_security_deny_hosts),
            adblockEngineHandle: ui.blocking_adblock_handle
        })
        profile.spellCheckEnabled=rootWindow.spellcheckEnabled(ui)
        profile.spellCheckLanguages=rootWindow.spellcheckLanguages(ui)
        profile.isPushServiceEnabled=rootWindow.pushServiceEnabled(ui,transientProfile)
    }
    onPolicyUiChanged: refreshPolicy()
    property Connections policyConnection: Connections {
        target: resources.policyUi
        function onProfile_bootstrap_pendingChanged() { resources.refreshPolicy() }
        function onBlocking_policy_readyChanged() { resources.refreshPolicy() }
        function onBlocking_enabledChanged() { resources.refreshPolicy() }
        function onBlocking_hostsChanged() { resources.refreshPolicy() }
        function onBlocking_exceptionsChanged() { resources.refreshPolicy() }
        function onBlocking_rule_hostsChanged() { resources.refreshPolicy() }
        function onBlocking_rule_list_idsChanged() { resources.refreshPolicy() }
        function onBlocking_exception_rule_hostsChanged() { resources.refreshPolicy() }
        function onBlocking_exception_rule_list_idsChanged() { resources.refreshPolicy() }
        function onBlocking_bypass_sitesChanged() { resources.refreshPolicy() }
        function onBlocking_security_deny_hostsChanged() { resources.refreshPolicy() }
        function onBlocking_adblock_handleChanged() { resources.refreshPolicy() }
        function onConfig_jsonChanged() { resources.refreshPolicy() }
    }

    function retireView(view) {
        if (!view) return
        if (view.viewRetired !== undefined) view.viewRetired=true
        if (view.viewModel !== undefined) view.viewModel=({loaded:false,url:"",muted:false,zoom:1.0})
        var host=view.viewHost
        if (host && host.tabViews && host.tabViews.indexOf(view) >= 0)
            host.tabViews=host.tabViews.filter(function(candidate) { return candidate!==view })
    }

    property Connections profileConnection: Connections {
        target: resources.profile
        function onDownloadRequested(download) {
            resources.rootWindow.profileResourceRegistry.handleDownloadRequested(resources,download)
        }
        function onDownloadFinished(download) {
            resources.rootWindow.profileResourceRegistry.handleDownloadFinished(resources,download)
        }
        function onPresentNotification(notification) {
            if (resources.policyUi)
                resources.rootWindow.presentWebNotification(resources.policyUi,notification,resources.transientProfile)
        }
    }

    Component.onCompleted: {
        if (existingProfile) {
            profile=existingProfile
            profilePrototype=existingPrototype
        } else {
            profilePrototype=profileFactory.createObject(resources)
            if (profilePrototype) profile=profilePrototype.instance()
        }
        if (!existingInterceptor) managedInterceptor=interceptorFactory.createObject(resources)
        if (!profile || !profilePrototype || !interceptor) return
        usable=lifetime.ownProfile(profilePrototype,profile) && lifetime.ownResource(interceptor)
               && interceptor.attach(profile)
    }
}
