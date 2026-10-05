package dev.burnlang.intellij;

import com.intellij.notification.NotificationGroupManager;
import com.intellij.notification.NotificationType;
import com.intellij.openapi.actionSystem.ActionUpdateThread;
import com.intellij.openapi.actionSystem.AnAction;
import com.intellij.openapi.actionSystem.AnActionEvent;
import com.intellij.openapi.application.ApplicationManager;
import com.intellij.openapi.fileEditor.FileEditorManager;
import com.intellij.openapi.project.Project;
import com.intellij.openapi.ui.popup.JBPopupFactory;
import com.intellij.openapi.vfs.LocalFileSystem;
import com.intellij.openapi.vfs.VirtualFile;
import org.jetbrains.annotations.NotNull;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.stream.Stream;

public abstract class OpenLibrarySourceAction extends AnAction {
    @Override
    public @NotNull ActionUpdateThread getActionUpdateThread() {
        return ActionUpdateThread.BGT;
    }

    @Override
    public void update(@NotNull AnActionEvent e) {
        e.getPresentation().setEnabledAndVisible(e.getProject() != null);
    }

    @Override
    public void actionPerformed(@NotNull AnActionEvent e) {
        Project project = e.getProject();
        if (project == null) {
            return;
        }
        ApplicationManager.getApplication().executeOnPooledThread(() -> {
            Path dir = BurnToolchain.sourcesDir();
            ApplicationManager.getApplication().invokeLater(() -> {
                if (project.isDisposed()) {
                    return;
                }
                if (dir == null) {
                    NotificationGroupManager.getInstance()
                            .getNotificationGroup("Burn")
                            .createNotification("Could not run `burn sources`. Is Burn installed?", NotificationType.ERROR)
                            .notify(project);
                    return;
                }
                open(project, dir);
            });
        });
    }

    protected abstract void open(@NotNull Project project, @NotNull Path dir);

    static void openFile(@NotNull Project project, @NotNull Path path) {
        VirtualFile file = LocalFileSystem.getInstance().refreshAndFindFileByNioFile(path);
        if (file != null) {
            FileEditorManager.getInstance(project).openFile(file, true);
        }
    }

    public static final class Builtins extends OpenLibrarySourceAction {
        @Override
        protected void open(@NotNull Project project, @NotNull Path dir) {
            openFile(project, dir.resolve("builtins.bn"));
        }
    }

    public static final class Stdlib extends OpenLibrarySourceAction {
        @Override
        protected void open(@NotNull Project project, @NotNull Path dir) {
            Path std = dir.resolve("std");
            List<String> modules;
            try (Stream<Path> files = Files.list(std)) {
                modules = files
                        .map(p -> p.getFileName().toString())
                        .filter(n -> n.endsWith(".bn"))
                        .map(n -> n.substring(0, n.length() - 3))
                        .sorted()
                        .toList();
            } catch (IOException ex) {
                return;
            }
            JBPopupFactory.getInstance()
                    .createPopupChooserBuilder(modules)
                    .setTitle("Burn Standard Library")
                    .setItemChosenCallback(m -> openFile(project, std.resolve(m + ".bn")))
                    .createPopup()
                    .showCenteredInCurrentWindow(project);
        }
    }
}
