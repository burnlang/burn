package dev.burnlang.intellij;

import com.intellij.openapi.actionSystem.AnActionEvent;
import com.intellij.openapi.actionSystem.CommonDataKeys;
import com.intellij.openapi.vfs.VirtualFile;
import org.jetbrains.annotations.NotNull;
import org.jetbrains.annotations.Nullable;

final class BurnFiles {
    private BurnFiles() {
    }

    static @Nullable VirtualFile current(@NotNull AnActionEvent e) {
        VirtualFile file = e.getData(CommonDataKeys.VIRTUAL_FILE);
        if (file == null || file.isDirectory() || !"bn".equals(file.getExtension())) {
            return null;
        }
        return file;
    }
}
