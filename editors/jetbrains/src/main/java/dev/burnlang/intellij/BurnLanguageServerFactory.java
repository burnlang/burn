package dev.burnlang.intellij;

import com.intellij.openapi.project.Project;
import com.redhat.devtools.lsp4ij.LanguageServerFactory;
import com.redhat.devtools.lsp4ij.server.OSProcessStreamConnectionProvider;
import com.redhat.devtools.lsp4ij.server.StreamConnectionProvider;
import org.jetbrains.annotations.NotNull;

public final class BurnLanguageServerFactory implements LanguageServerFactory {
    @Override
    public @NotNull StreamConnectionProvider createConnectionProvider(@NotNull Project project) {
        return new OSProcessStreamConnectionProvider(BurnToolchain.command(project.getBasePath(), "lsp"));
    }
}
