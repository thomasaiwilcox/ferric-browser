#ifndef FERRIC_BROWSER_SETTINGS_MODEL_H
#define FERRIC_BROWSER_SETTINGS_MODEL_H

#include <QtCore/QAbstractListModel>
#include <QtCore/QStringList>
#include <QtQml/qqmlregistration.h>

class FerricBrowserSettingsModel : public QAbstractListModel {
    Q_OBJECT
    QML_NAMED_ELEMENT(FerricSettingsModel)

public:
    enum Role { KeyRole = Qt::UserRole + 1, LabelRole, TypeRole, ScopeRole, ApplyRole, ValueRole, OptionsRole };
    Q_ENUM(Role)

    explicit FerricBrowserSettingsModel(QObject *parent = nullptr);
    int rowCount(const QModelIndex &parent = {}) const override;
    QVariant data(const QModelIndex &index, int role) const override;
    QHash<int, QByteArray> roleNames() const override;

    Q_INVOKABLE bool replaceRows(const QStringList &keys, const QStringList &labels,
                                 const QStringList &types, const QStringList &scopes,
                                 const QStringList &applies, const QStringList &values,
                                 const QStringList &options);

private:
    struct Row { QString key; QString label; QString type; QString scope; QString apply; QString value; QStringList options; };
    QList<Row> rows_;
};

#endif
