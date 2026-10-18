#ifndef FERRIC_GUI_APPLICATION_H
#define FERRIC_GUI_APPLICATION_H
#include <QByteArray>
#include <QGuiApplication>
#include <QVector>
#include <memory>

std::unique_ptr<QGuiApplication> ferric_browser_gui_application(const QVector<QByteArray> &arguments);
bool ferric_browser_gui_preferences_ready();
#endif
