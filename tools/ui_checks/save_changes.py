#!/usr/bin/env python3
"""Check the Save Changes shortcut under Xvfb. Usage: save_changes.py build/Aztek/AppRun."""
import os
from pathlib import Path
import subprocess
import sys
import tempfile


app = Path(sys.argv[1]).resolve()
with tempfile.TemporaryDirectory(prefix="aztek_save_changes_") as directory:
    work = Path(directory)
    script = work / "check.py"
    script.write_text("""
import os
from traceback import format_exc

from maya import cmds
from PySide6.QtCore import Qt, QTimer
from PySide6.QtTest import QTest
from PySide6.QtWidgets import QApplication, QMessageBox

app = QApplication.instance()
main = next(w for w in app.topLevelWidgets() if w.objectName() == "MayaWindow")
dialog_seen = False

def discard():
    global dialog_seen
    dialog = app.activeModalWidget()
    if isinstance(dialog, QMessageBox) and dialog.windowTitle() == "Save Changes":
        dialog_seen = True
        QApplication.setActiveWindow(dialog)
        QTest.keyClick(dialog, Qt.Key_N)
        QTest.qWait(150)
        if dialog.isVisible():
            dialog.reject()

try:
    cube, _ = cmds.polyCube()
    QApplication.setActiveWindow(main)
    QTimer.singleShot(100, discard)
    file_action = next(a for a in main.menuBar().actions() if a.text() == "File")
    file_menu = file_action.menu()
    next(a for a in file_menu.actions() if a.text() == "New Scene").trigger()
    assert dialog_seen, "New Scene did not open Save Changes"
    assert not cmds.objExists(cube), "N did not discard the unsaved scene"
except Exception:
    os.write(2, format_exc().encode())
    app.exit(1)
else:
    main.close()
""")
    env = dict(os.environ, XDG_CONFIG_HOME=str(work / "config"),
               AZTEK_PREFS=str(work / "prefs"), AZTEK_APP_DIR=str(work / "aztek"))
    subprocess.run([str(app), "--script", str(script)], env=env, check=True, timeout=20)
print("N discarded the unsaved scene.")
