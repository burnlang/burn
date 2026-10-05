plugins {
    java
    id("org.jetbrains.intellij.platform") version "2.19.0"
}

group = "dev.burnlang"
version = providers.gradleProperty("pluginVersion").get()

repositories {
    mavenCentral()
    intellijPlatform {
        defaultRepositories()
    }
}

dependencies {
    intellijPlatform {
        intellijIdeaCommunity(providers.gradleProperty("platformVersion"))
        bundledPlugin("org.jetbrains.plugins.textmate")
        plugin("com.redhat.devtools.lsp4ij", providers.gradleProperty("lsp4ijVersion").get())
    }
}

java {
    toolchain {
        languageVersion = JavaLanguageVersion.of(21)
    }
}

intellijPlatform {
    pluginConfiguration {
        version = providers.gradleProperty("pluginVersion")
        ideaVersion {
            sinceBuild = "242"
            untilBuild = provider { null }
        }
    }
    buildSearchableOptions = false
}

tasks.processResources {
    from("../vscode") {
        include("package.json", "language-configuration.json", "syntaxes/**", "snippets/**")
        into("textmate")
    }
}
