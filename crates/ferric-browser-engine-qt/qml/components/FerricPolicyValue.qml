import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../scripts/SitePolicyEditorSchema.js" as PolicySchema

// Recursive, closed-schema native controls. Scalar edits mutate the local
// proposal in place; receipt refreshes never replace these controls or undo.
Item {
    id: editor
    property var spec: ({type:""})
    property var container: ({})
    property var fieldKey: ""
    property string fieldPath: ""
    property bool readOnly: false
    property var errors: ({})
    readonly property var value: container ? container[fieldKey] : undefined
    signal edited()
    signal issue(string path, string message, bool subtree)
    implicitHeight: control.item ? control.item.implicitHeight : 0
    function changed(value) { container[fieldKey] = value; edited() }
    function connectChild(loader, object, key, childSpec, path) {
        loader.item.container = object
        loader.item.fieldKey = key
        loader.item.fieldPath = path
        loader.item.readOnly = Qt.binding(function() { return editor.readOnly })
        loader.item.errors = Qt.binding(function() { return editor.errors })
        loader.item.edited.connect(function() { editor.edited() })
        loader.item.issue.connect(function(path, message, subtree) { editor.issue(path, message, subtree) })
        loader.item.spec = childSpec
    }
    Loader {
        id: control
        width: parent.width
        active: editor.spec.type.length > 0
        sourceComponent: PolicySchema.types[editor.spec.type] ? objectControl
            : editor.spec.type === "list" ? listControl
            : editor.spec.type === "bool" ? booleanControl
            : PolicySchema.choices(editor.spec.type).length > 0 ? enumControl : textControl
    }
    Component {
        id: textControl
        FerricTextField {
            objectName: "policyField:" + editor.fieldPath
            width: editor.width
            readOnly: editor.readOnly
            Accessible.name: editor.spec.label || editor.fieldPath
            Accessible.description: editor.spec.help || ""
            selectByMouse: true
            persistentSelection: true
            Component.onCompleted: text = editor.value === undefined ? "" : String(editor.value)
            onTextEdited: {
                var numeric = editor.spec.type === "number" || editor.spec.type === "integer"
                if (!numeric) { editor.changed(text); return }
                var number = Number(text)
                var valid = text.trim().length > 0 && isFinite(number)
                    && (editor.spec.type !== "integer" || (Math.floor(number) === number && Math.abs(number) <= 9007199254740991))
                editor.issue(editor.fieldPath, valid ? "" : "Enter a finite " + (editor.spec.type === "integer" ? "integer" : "number"), false)
                if (valid) editor.changed(number)
                else editor.edited()
            }
        }
    }
    Component {
        id: booleanControl
        CheckBox {
            objectName: "policyField:" + editor.fieldPath
            enabled: !editor.readOnly
            text: checked ? "True" : "False"
            Accessible.name: editor.spec.label || editor.fieldPath
            Accessible.description: editor.spec.help || ""
            Component.onCompleted: checked = editor.value === true
            onToggled: editor.changed(checked)
        }
    }
    Component {
        id: enumControl
        ComboBox {
            objectName: "policyField:" + editor.fieldPath
            width: editor.width
            enabled: !editor.readOnly
            model: PolicySchema.choices(editor.spec.type)
            Accessible.name: editor.spec.label || editor.fieldPath
            Accessible.description: editor.spec.help || ""
            Component.onCompleted: currentIndex = Math.max(0, model.indexOf(editor.value))
            onActivated: {
                PolicySchema.choose(editor.container, editor.container.scope !== undefined ? "receiver" : "", editor.fieldKey, currentText)
                editor.edited()
            }
        }
    }
    Component {
        id: objectControl
        Column {
            id: objectFields
            width: editor.width
            spacing: 10
            property int contextRevision: 0
            Connections { target: editor; function onEdited() { objectFields.contextRevision++ } }
            Repeater {
                model: PolicySchema.fields(editor.spec.type)
                delegate: Column {
                    id: fieldRow
                    required property var modelData
                    width: objectFields.width
                    spacing: 4
                    readonly property var fieldSpec: modelData
                    readonly property string path: editor.fieldPath + "." + fieldSpec.key
                    readonly property bool applicable: {
                        objectFields.contextRevision
                        return PolicySchema.applicable(fieldSpec, editor.value || {})
                    }
                    property bool included: fieldSpec.required || PolicySchema.present(editor.value, fieldSpec.key)
                    property bool expanded: !PolicySchema.types[fieldSpec.type] && fieldSpec.type !== "list"
                    property bool loadedOnce: expanded
                    visible: applicable
                    RowLayout {
                        width: parent.width
                        CheckBox {
                            visible: !fieldRow.fieldSpec.required
                            enabled: !editor.readOnly
                            objectName: "policyInclude:" + fieldRow.path
                            checked: fieldRow.included
                            text: "Include " + fieldRow.fieldSpec.label
                            Accessible.description: "Unchecked omits this field; a present empty list explicitly clears lower values."
                            onToggled: {
                                fieldRow.included = checked
                                if (checked) editor.value[fieldRow.fieldSpec.key] = PolicySchema.defaultValue(fieldRow.fieldSpec.type)
                                else { delete editor.value[fieldRow.fieldSpec.key]; editor.issue(fieldRow.path, "", true) }
                                editor.edited()
                            }
                        }
                        Label {
                            visible: fieldRow.fieldSpec.required
                            text: fieldRow.fieldSpec.label
                            textFormat: Text.PlainText
                            font.bold: true
                            Layout.fillWidth: true
                            wrapMode: Text.Wrap
                        }
                        ToolButton {
                            objectName: "policyExpand:" + fieldRow.path
                            visible: PolicySchema.types[fieldRow.fieldSpec.type] !== undefined || fieldRow.fieldSpec.type === "list"
                            enabled: fieldRow.included
                            text: fieldRow.expanded ? "Collapse" : "Edit"
                            Accessible.name: text + " " + fieldRow.fieldSpec.label
                            onClicked: { fieldRow.expanded = !fieldRow.expanded; fieldRow.loadedOnce = true }
                        }
                    }
                    Label {
                        width: parent.width
                        text: fieldRow.fieldSpec.help
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        opacity: 0.72
                    }
                    Loader {
                        id: child
                        width: parent.width
                        visible: fieldRow.included && fieldRow.expanded
                        active: fieldRow.applicable && fieldRow.included && fieldRow.loadedOnce
                        source: active ? Qt.resolvedUrl("FerricPolicyValue.qml") : ""
                        onLoaded: editor.connectChild(child, editor.value, fieldRow.fieldSpec.key, fieldRow.fieldSpec, fieldRow.path)
                    }
                }
            }
        }
    }
    Component {
        id: listControl
        Column {
            id: list
            width: editor.width
            spacing: 8
            property var indices: []
            // Rebuilding indices recreates controls from valid proposals. Keep
            // unfinished numeric text in place until it can be committed.
            readonly property bool hasInvalidInput: Object.keys(editor.errors).some(function(path) {
                return path === editor.fieldPath || path.indexOf(editor.fieldPath + ".") === 0
            })
            function rebuild() {
                var rows = []
                for (var index = 0; index < (editor.value || []).length; ++index) rows.push(index)
                indices = rows
                editor.issue(editor.fieldPath, "", true)
            }
            function move(index, offset) {
                var values = editor.value
                var next = index + offset
                if (next < 0 || next >= values.length) return
                var temporary = values[index]; values[index] = values[next]; values[next] = temporary
                rebuild(); editor.edited()
            }
            Component.onCompleted: rebuild()
            Label {
                text: list.indices.length === 0 ? "Empty list: explicitly clears lower values" : "Entries retain this order"
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                width: parent.width
            }
            Repeater {
                model: list.indices
                delegate: Frame {
                    id: entry
                    required property int modelData
                    width: list.width
                    Column {
                        width: parent.width
                        spacing: 6
                        RowLayout {
                            width: parent.width
                            Label { text: "Entry " + (entry.modelData + 1); Layout.fillWidth: true }
                            ToolButton { objectName: "policyMoveUp:" + editor.fieldPath + "." + entry.modelData; text: "↑"; enabled: !editor.readOnly && !list.hasInvalidInput && entry.modelData > 0; Accessible.name: "Move entry up"; onClicked: list.move(entry.modelData, -1) }
                            ToolButton { objectName: "policyMoveDown:" + editor.fieldPath + "." + entry.modelData; text: "↓"; enabled: !editor.readOnly && !list.hasInvalidInput && entry.modelData + 1 < list.indices.length; Accessible.name: "Move entry down"; onClicked: list.move(entry.modelData, 1) }
                            ToolButton {
                                objectName: "policyRemove:" + editor.fieldPath + "." + entry.modelData
                                text: "Remove"; enabled: !editor.readOnly && !list.hasInvalidInput
                                Accessible.name: "Remove entry " + (entry.modelData + 1)
                                onClicked: { editor.value.splice(entry.modelData, 1); list.rebuild(); editor.edited() }
                            }
                        }
                        Loader {
                            id: listValue
                            width: parent.width
                            source: Qt.resolvedUrl("FerricPolicyValue.qml")
                            onLoaded: editor.connectChild(listValue, editor.value, entry.modelData,
                                {type:editor.spec.item, label:editor.spec.label + " entry " + (entry.modelData + 1), required:true},
                                editor.fieldPath + "." + entry.modelData)
                        }
                    }
                }
            }
            RowLayout {
                Button {
                    text: "Add entry"; enabled: !editor.readOnly && !list.hasInvalidInput && list.indices.length < editor.spec.maximum
                    objectName: "policyAdd:" + editor.fieldPath
                    onClicked: { editor.value.push(PolicySchema.defaultValue(editor.spec.item)); list.rebuild(); editor.edited() }
                }
                Button {
                    text: "Clear list"; enabled: !editor.readOnly && list.indices.length > 0
                    objectName: "policyClear:" + editor.fieldPath
                    onClicked: { editor.value.splice(0, editor.value.length); list.rebuild(); editor.edited() }
                }
            }
        }
    }
}
