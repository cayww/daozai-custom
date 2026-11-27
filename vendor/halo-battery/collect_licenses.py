"""把运行库及冻结工具的许可和上游归属放入组件。"""

from importlib.metadata import distribution
from pathlib import Path
import shutil
import sys


source = Path(__file__).resolve().parent
target = Path(sys.argv[1])
target.mkdir(parents=True, exist_ok=True)
shutil.copyfile(source / "upstream" / "LICENSE", target / "HaloBattery-LICENSE.txt")
shutil.copyfile(source / "upstream" / "docs" / "credits.md", target / "HaloBattery-credits.md")
shutil.copyfile(Path(sys.base_prefix) / "LICENSE.txt", target / "Python-LICENSE.txt")

for project in (
    "altgraph", "packaging", "pefile", "pyinstaller", "pyinstaller-hooks-contrib",
    "pywin32-ctypes", "setuptools",
):
    dist = distribution(project)
    licenses = target / project
    licenses.mkdir(exist_ok=True)
    found = False
    for file in dist.files or ():
        basename = file.name.casefold()
        if basename.startswith(("copying", "license", "notice")) and ".dist-info" in file.as_posix():
            shutil.copyfile(dist.locate_file(file), licenses / file.name)
            found = True
    # PyInstaller 的许可含 bootloader 链接例外，必须随 onefile 发布。
    if project == "pyinstaller" and not found:
        raise OSError("PyInstaller 许可文件缺失")
