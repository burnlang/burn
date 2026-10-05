package dev.burnlang.intellij;

import com.intellij.openapi.application.PathManager;
import org.jetbrains.annotations.NotNull;
import org.jetbrains.plugins.textmate.api.TextMateBundleProvider;

import java.io.IOException;
import java.io.InputStream;
import java.io.UncheckedIOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.List;

public final class BurnTextMateBundleProvider implements TextMateBundleProvider {
    private static final List<String> FILES = List.of(
            "package.json",
            "language-configuration.json",
            "syntaxes/burn.tmLanguage.json",
            "snippets/burn.json");

    @Override
    public @NotNull List<PluginBundle> getBundles() {
        return List.of(new PluginBundle("Burn", unpack()));
    }

    private static Path unpack() {
        Path target = Path.of(PathManager.getSystemPath(), "burn", "textmate");
        try {
            for (String name : FILES) {
                try (InputStream in = BurnTextMateBundleProvider.class.getResourceAsStream("/textmate/" + name)) {
                    if (in == null) {
                        continue;
                    }
                    Path file = target.resolve(name);
                    Files.createDirectories(file.getParent());
                    Files.copy(in, file, StandardCopyOption.REPLACE_EXISTING);
                }
            }
        } catch (IOException e) {
            throw new UncheckedIOException(e);
        }
        return target;
    }
}
