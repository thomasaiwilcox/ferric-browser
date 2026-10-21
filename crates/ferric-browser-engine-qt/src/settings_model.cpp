#include "settings_model.h"

#include <QtCore/QMetaType>
#include <QtCore/QSet>
#include <QtCore/QVariantMap>

FerricBrowserSettingsModel::FerricBrowserSettingsModel(QObject *parent) : QAbstractListModel(parent) {}
int FerricBrowserSettingsModel::rowCount(const QModelIndex &parent) const { return parent.isValid() ? 0 : rows_.size(); }
QVariant FerricBrowserSettingsModel::data(const QModelIndex &index, int role) const {
    if (!index.isValid() || index.row() < 0 || index.row() >= rows_.size()) return {};
    const auto &row = rows_.at(index.row());
    switch (role) { case KeyRole: return row.key; case LabelRole: return row.label; case TypeRole: return row.type;
    case ScopeRole: return row.scope; case ApplyRole: return row.apply; case ValueRole: return row.value;
    case OptionsRole: return row.options; default: return {}; }
}
QHash<int, QByteArray> FerricBrowserSettingsModel::roleNames() const {
    return {{KeyRole,"key"},{LabelRole,"label"},{TypeRole,"type"},{ScopeRole,"scope"},{ApplyRole,"apply"},{ValueRole,"value"},{OptionsRole,"options"}};
}
bool FerricBrowserSettingsModel::replaceRows(const QVariantList &values) {
    if (values.size() > 128) {
        return false;
    }
    QList<Row> rows;
    rows.reserve(values.size());
    QSet<QString> keys;
    const QStringList fields = {"key", "label", "type", "scope", "apply", "value", "options"};
    for (const QVariant &value : values) {
        if (value.metaType().id() != QMetaType::QVariantMap) {
            return false;
        }
        const QVariantMap row = value.toMap();
        if (row.size() != fields.size()) {
            return false;
        }
        for (const QString &field : fields) {
            if (!row.contains(field)) {
                return false;
            }
        }
        if (row.value("key").metaType().id() != QMetaType::QString ||
            row.value("label").metaType().id() != QMetaType::QString ||
            row.value("type").metaType().id() != QMetaType::QString ||
            row.value("scope").metaType().id() != QMetaType::QString ||
            row.value("apply").metaType().id() != QMetaType::QString ||
            row.value("value").metaType().id() != QMetaType::QString ||
            row.value("options").metaType().id() != QMetaType::QStringList) {
            return false;
        }
        const auto key = row.value("key").toString();
        if (key.isEmpty() || keys.contains(key)) return false;
        keys.insert(key);
        rows.append({key, row.value("label").toString(),
                     row.value("type").toString(), row.value("scope").toString(),
                     row.value("apply").toString(), row.value("value").toString(),
                     row.value("options").toStringList()});
    }
    // Keep surviving editors alive: resetting the model discards their
    // selection, undo history and keyboard focus on every settings refresh.
    for (int index = 0; index < rows.size(); ++index) {
        int previous = index;
        while (previous < rows_.size() && rows_[previous].key != rows[index].key) ++previous;
        if (previous == rows_.size()) {
            beginInsertRows({}, index, index);
            rows_.insert(index, rows[index]);
            endInsertRows();
            continue;
        }
        if (previous != index) {
            beginMoveRows({}, previous, previous, {}, index);
            rows_.move(previous, index);
            endMoveRows();
        }
        auto &current = rows_[index];
        const auto &next = rows[index];
        QList<int> changed;
        const auto update = [&changed](auto &field, const auto &value, int role) {
            if (field != value) { field = value; changed.append(role); }
        };
        update(current.label, next.label, LabelRole);
        update(current.type, next.type, TypeRole);
        update(current.scope, next.scope, ScopeRole);
        update(current.apply, next.apply, ApplyRole);
        update(current.value, next.value, ValueRole);
        update(current.options, next.options, OptionsRole);
        if (!changed.isEmpty()) emit dataChanged(this->index(index), this->index(index), changed);
    }
    if (rows_.size() > rows.size()) {
        beginRemoveRows({}, rows.size(), rows_.size() - 1);
        rows_.remove(rows.size(), rows_.size() - rows.size());
        endRemoveRows();
    }
    return true;
}
