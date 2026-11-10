"""Resolve the user's Desktop, including Windows redirected Desktop folders."""
from pathlib import Path
import os

def desktop_directory():
    if os.name=='nt':
        import ctypes
        buffer=ctypes.create_unicode_buffer(32768)
        if ctypes.windll.shell32.SHGetFolderPathW(None,0x10,None,0,buffer)==0 and buffer.value:
            return Path(buffer.value)
    return Path.home()/'Desktop'
