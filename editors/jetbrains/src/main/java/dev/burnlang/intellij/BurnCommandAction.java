package dev.burnlang.intellij;

import com.intellij.execution.ExecutionException;
import com.intellij.execution.RunContentExecutor;
import com.intellij.execution.configurations.GeneralCommandLine;
import com.intellij.execution.process.OSProcessHandler;
import com.intellij.execution.process.ProcessHandlerFactory;
import com.intellij.execution.process.ProcessTerminatedListener;
import com.intellij.notification.NotificationGroupManager;
import com.intellij.notification.NotificationType;
import com.intellij.openapi.actionSystem.ActionUpdateThread;
import com.intellij.openapi.actionSystem.AnAction;
import com.intellij.openapi.actionSystem.AnActionEvent;
import com.intellij.openapi.fileEditor.FileDocumentManager;
import com.intellij.openapi.project.Project;
import com.intellij.openapi.vfs.VirtualFile;
import org.jetbrains.annotations.NotNull;

import java.util.ArrayList;
import java.util.List;

public abstract class BurnCommandAction extends AnAction {
    private final String title;
    private final List<String> args;

    protected BurnCommandAction(String title, String... args) {
        this.title = title;
        this.args = List.of(args);
    }

    @Override
    public @NotNull ActionUpdateThread getActionUpdateThread() {
        return ActionUpdateThread.BGT;
    }

    @Override
    public void update(@NotNull AnActionEvent e) {
        e.getPresentation().setEnabledAndVisible(e.getProject() != null && BurnFiles.current(e) != null);
    }

    @Override
    public void actionPerformed(@NotNull AnActionEvent e) {
        Project project = e.getProject();
        VirtualFile file = BurnFiles.current(e);
        if (project == null || file == null) {
            return;
        }
        FileDocumentManager.getInstance().saveAllDocuments();
        List<String> all = new ArrayList<>(args);
        all.add(file.getPath());
        VirtualFile dir = file.getParent();
        GeneralCommandLine cmd = BurnToolchain.command(dir == null ? null : dir.getPath(), all.toArray(String[]::new));
        try {
            OSProcessHandler handler = ProcessHandlerFactory.getInstance().createColoredProcessHandler(cmd);
            ProcessTerminatedListener.attach(handler);
            new RunContentExecutor(project, handler).withTitle(title + " " + file.getName()).run();
        } catch (ExecutionException ex) {
            NotificationGroupManager.getInstance()
                    .getNotificationGroup("Burn")
                    .createNotification("Could not start burn: " + ex.getMessage(), NotificationType.ERROR)
                    .notify(project);
        }
    }

    public static final class Run extends BurnCommandAction {
        public Run() {
            super("burn run", "run");
        }
    }

    public static final class RunNative extends BurnCommandAction {
        public RunNative() {
            super("burn run --native", "run", "--native");
        }
    }

    public static final class Build extends BurnCommandAction {
        public Build() {
            super("burn build", "build");
        }
    }
}
