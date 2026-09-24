#include "settings_model.h"

#include <algorithm>

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
void FerricBrowserSettingsModel::replaceRows(const QStringList &keys, const QStringList &labels,
                                             const QStringList &types, const QStringList &scopes,
                                             const QStringList &applies, const QStringList &values,
                                             const QStringList &options) {
    const auto count = std::min({keys.size(), labels.size(), types.size(), scopes.size(), applies.size(), values.size(), options.size(), qsizetype{128}});
    beginResetModel(); rows_.clear(); rows_.reserve(count);
    for (qsizetype i = 0; i < count; ++i) rows_.append({keys.at(i), labels.at(i), types.at(i), scopes.at(i), applies.at(i), values.at(i), options.at(i).split(QChar(0x1f), Qt::SkipEmptyParts)});
    endResetModel();
}
