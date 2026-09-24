#include "settings_model.h"

#include <QtCore/QMetaType>
#include <QtCore/QVariantMap>

#include <utility>

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
        rows.append({row.value("key").toString(), row.value("label").toString(),
                     row.value("type").toString(), row.value("scope").toString(),
                     row.value("apply").toString(), row.value("value").toString(),
                     row.value("options").toStringList()});
    }
    beginResetModel(); rows_ = std::move(rows);
    endResetModel();
    return true;
}
