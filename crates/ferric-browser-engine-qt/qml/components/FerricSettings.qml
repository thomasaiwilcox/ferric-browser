import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

FerricModalSurface {
    id: settings
    objectName: "settingsSurface"
    required property bool settingsVisible
    required property bool temporary
    required property string searchText
    required property string notice
    readonly property bool filtering: searchText.trim().length > 0
    property string noticeDetail: ""
    property bool noticeFailed: false
    property bool noticePending: false
    property string profileLabel: ""
    property bool transientProfile: browserWindow.temporaryProfile === true || browserWindow.ephemeralProfile === true
    required property string spellcheckStatus
    required property bool spellcheckDictionariesMissing
    required property var userscriptRows
    required property var settingsModel
    signal closeRequested
    signal refreshRequested
    signal installUserscriptRequested
    signal temporaryChangeRequested(bool temporary)
    signal searchChanged(string text)
    signal userscriptEnabledRequested(string name, bool enabled)
    signal userscriptRemovalRequested(string name)
    signal settingApplyRequested(var row, var value)
    signal settingResetRequested(var row)

    function setUserscriptEnabled(name, enabled) {
        userscriptInventory.setEnabled(name, enabled);
    }
    function clearUserscriptOverrides() {
        userscriptInventory.clearEnabledOverrides();
    }

    visible: settingsVisible
    commandText: ":settings"
    title: "Settings"
    message: profileLabel.length > 0 ? "Profile: " + profileLabel : "Browsing and interface preferences"
    keyHelp: "type to filter  ·  tab/shift-tab controls  ·  esc close"
    maximumSpacingScale: Math.max(1, Math.min(width / 800, height / 650))
    dialogWidth: 980 * scale
    dialogHeight: 680 * scale
    stackingOrder: 76
    initialFocusItem: sections.currentIndex === 0 ? settingsSearchInput : scriptInstall
    onSearchTextChanged: if (searchText.length > 0)
        sections.currentIndex = 0
    onDismissRequested: closeRequested()

    ColumnLayout {
        anchors.fill: parent
        spacing: 8 * settings.scale

        TabBar {
            id: sections
            Layout.fillWidth: true
            TabButton {
                objectName: "settingsPreferencesTab"
                text: "Settings"
            }
            TabButton {
                objectName: "settingsScriptsTab"
                text: "Scripts"
                Accessible.name: "Installed userscripts"
            }
        }
        FerricScrollColumn {
            id: settingsScroll
            objectName: "settingsScroll"
            visible: sections.currentIndex === 0
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.minimumHeight: 0
            hostWindow: settings.hostWindow
            FerricTextField {
                id: settingsSearchInput
                objectName: "settingsSearchInput"
                Layout.fillWidth: true
                text: settings.searchText
                placeholderText: "Search settings by name or configuration key"
                Accessible.name: "Search settings"
                Accessible.role: Accessible.EditableText
                onTextChanged: settings.searchChanged(text)
                Keys.onPressed: function (event) {
                    if (event.key === Qt.Key_Escape) {
                        settings.closeRequested();
                        event.accepted = true;
                    }
                }
            }

            ColumnLayout {
                Layout.fillWidth: true
                CheckBox {
                    id: sessionScope
                    Layout.fillWidth: true
                    Layout.minimumWidth: 0
                    text: "Session only"
                    checked: settings.temporary || settings.transientProfile
                    enabled: !settings.transientProfile
                    Accessible.name: "Apply settings temporarily"
                    Accessible.description: "Changes remain in memory for this session"
                    onToggled: settings.temporaryChangeRequested(checked)
                    contentItem: Label {
                        objectName: "settingsSessionScopeLabel"
                        text: sessionScope.text
                        font: sessionScope.font
                        color: settings.browserWindow.primaryTextColor
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        leftPadding: !sessionScope.mirrored && sessionScope.indicator ? sessionScope.indicator.width + sessionScope.spacing : 0
                        rightPadding: sessionScope.mirrored && sessionScope.indicator ? sessionScope.indicator.width + sessionScope.spacing : 0
                    }
                }
                Label {
                    visible: !settings.filtering
                    Layout.fillWidth: true
                    text: settings.transientProfile ? "Changes stay in memory and are forgotten when this private or temporary profile closes." : "Session changes are forgotten when Ferric closes. Turn this off to save changes; your config.toml file stays intact."
                    color: settings.browserWindow.secondaryTextColor
                    wrapMode: Text.WordWrap
                }
            }

            FerricSettingRows {
                id: settingRows
                Layout.fillWidth: true
                Layout.preferredHeight: implicitHeight
                browserWindow: settings.browserWindow
                settingsModel: settings.settingsModel
                onApplyRequested: function (row, value) {
                    settings.settingApplyRequested(row, value);
                }
                onResetRequested: function (row) {
                    settings.settingResetRequested(row);
                }
            }
            Label {
                objectName: "settingsEmptyResults"
                Layout.fillWidth: true
                visible: settingRows.count === 0
                text: settings.filtering ? "No settings match this search. Try a different name or configuration key." : "No settings are available. Refresh to try again."
                textFormat: Text.PlainText
                color: settings.browserWindow.secondaryTextColor
                wrapMode: Text.Wrap
            }
            Label {
                visible: !settings.filtering
                Layout.fillWidth: true
                text: "For settings that apply to one site, use the site-rule commands."
                color: settings.browserWindow.secondaryTextColor
                wrapMode: Text.WordWrap
            }

            Label {
                visible: !settings.filtering
                Layout.fillWidth: true
                text: settings.spellcheckStatus
                color: settings.spellcheckDictionariesMissing ? settings.browserWindow.warningColor : settings.browserWindow.mutedTextColor
                wrapMode: Text.WordWrap
                Accessible.name: "Spellcheck dictionary status"
            }
        }
        FerricScrollColumn {
            objectName: "settingsScriptsScroll"
            visible: sections.currentIndex === 1
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.minimumHeight: 0
            hostWindow: settings.hostWindow
            Label {
                Layout.fillWidth: true
                text: "Installed scripts are shared by regular profiles."
                textFormat: Text.PlainText
                color: settings.browserWindow.secondaryTextColor
                wrapMode: Text.Wrap
            }

            FerricUserscriptInventory {
                id: userscriptInventory
                Layout.fillWidth: true
                Layout.preferredHeight: implicitHeight
                browserWindow: settings.browserWindow
                changesAvailable: !settings.transientProfile && !settings.noticePending
                visible: !settings.transientProfile && settings.userscriptRows.length > 0
                rows: settings.userscriptRows
                onEnabledRequested: function (name, enabled) {
                    settings.userscriptEnabledRequested(name, enabled);
                }
                onRemovalRequested: function (name) {
                    settings.userscriptRemovalRequested(name);
                }
            }

            Label {
                Layout.fillWidth: true
                visible: settings.transientProfile
                text: "Userscript changes are unavailable in private and temporary profiles."
                color: settings.browserWindow.mutedTextColor
                wrapMode: Text.WordWrap
            }
            Label {
                Layout.fillWidth: true
                visible: !settings.transientProfile && settings.userscriptRows.length === 0
                text: "No userscripts installed. Choose Install script to add a manifest."
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: settings.browserWindow.secondaryTextColor
            }
        }
        FerricScrollColumn {
            objectName: "settingsFeedbackViewport"
            Layout.fillWidth: true
            Layout.minimumHeight: 0
            Layout.preferredHeight: Math.min(contentLayout.implicitHeight, settings.bodyItem.height * 0.3)
            Layout.maximumHeight: settings.bodyItem.height * 0.3
            visible: settings.notice.length > 0
            hostWindow: settings.hostWindow
            FerricManagerFeedback {
                Layout.fillWidth: true
                browserWindow: settings.browserWindow
                message: settings.notice
                detail: settings.noticeDetail
                failed: settings.noticeFailed
                pending: settings.noticePending
                Accessible.name: "Settings status"
            }
        }
        ColumnLayout {
            Layout.fillWidth: true
            Layout.minimumWidth: 0
            Flow {
                Layout.fillWidth: true
                Layout.minimumWidth: 0
                Layout.maximumWidth: settings.bodyItem.width
                Layout.preferredHeight: childrenRect.height
                spacing: 8
                FerricWrappingButton {
                    text: "Refresh"
                    Accessible.name: "Refresh settings"
                    onClicked: settings.refreshRequested()
                }
                FerricWrappingButton {
                    id: scriptInstall
                    objectName: "settingsInstallScript"
                    visible: sections.currentIndex === 1
                    text: "Install script"
                    enabled: !settings.transientProfile && !settings.noticePending
                    Accessible.name: "Install userscript manifest"
                    onClicked: settings.installUserscriptRequested()
                }
                FerricWrappingButton {
                    text: "Close"
                    Accessible.name: "Close settings"
                    onClicked: settings.closeRequested()
                }
            }
        }
    }
}
