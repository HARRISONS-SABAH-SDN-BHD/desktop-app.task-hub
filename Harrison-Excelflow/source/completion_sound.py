"""Nonblocking Windows playback of the bundled completion chime."""
def play_completion_sound(path):
    try:
        import winsound
        winsound.PlaySound(str(path),winsound.SND_FILENAME | winsound.SND_ASYNC | winsound.SND_NODEFAULT)
    except (ImportError, OSError, RuntimeError):
        # Audio must never interfere with a successfully generated report.
        pass
