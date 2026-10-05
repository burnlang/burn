package dev.burnlang.intellij;

import com.intellij.execution.ExecutionException;
import com.intellij.execution.configurations.GeneralCommandLine;
import com.intellij.execution.configurations.PathEnvironmentVariableUtil;
import com.intellij.execution.process.CapturingProcessHandler;
import com.intellij.execution.process.ProcessOutput;
import com.intellij.openapi.util.SystemInfo;
import org.jetbrains.annotations.NotNull;
import org.jetbrains.annotations.Nullable;

import java.io.File;
import java.nio.file.Files;
import java.nio.file.Path;

public final class BurnToolchain {
    private BurnToolchain() {
    }

    public static @NotNull String executable() {
        String name = SystemInfo.isWindows ? "burn.exe" : "burn";
        File onPath = PathEnvironmentVariableUtil.findInPath(name);
        if (onPath != null) {
            return onPath.getAbsolutePath();
        }
        String home = System.getenv("BURN_HOME");
        if (home != null && !home.isEmpty()) {
            Path candidate = Path.of(home, "bin", name);
            if (Files.isExecutable(candidate)) {
                return candidate.toString();
            }
        }
        Path candidate = Path.of(System.getProperty("user.home"), ".burn", "bin", name);
        if (Files.isExecutable(candidate)) {
            return candidate.toString();
        }
        return name;
    }

    public static @NotNull GeneralCommandLine command(@Nullable String workDir, String... args) {
        GeneralCommandLine cmd = new GeneralCommandLine(executable());
        cmd.addParameters(args);
        cmd.withParentEnvironmentType(GeneralCommandLine.ParentEnvironmentType.CONSOLE);
        cmd.withCharset(java.nio.charset.StandardCharsets.UTF_8);
        if (workDir != null) {
            cmd.setWorkDirectory(workDir);
        }
        return cmd;
    }

    public static @Nullable Path sourcesDir() {
        try {
            ProcessOutput out = new CapturingProcessHandler(command(null, "sources")).runProcess(30_000);
            if (out.getExitCode() != 0) {
                return null;
            }
            String dir = out.getStdout().trim();
            return dir.isEmpty() ? null : Path.of(dir);
        } catch (ExecutionException e) {
            return null;
        }
    }
}
