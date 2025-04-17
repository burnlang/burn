package stdlib

import (
	"os"
	"path/filepath"
	"strings"
)

type Library struct {
	Name    string
	Content string
}

var RegisteredLibs = []Library{
	{Name: "date", Content: DateLib},
	{Name: "http", Content: HTTPLib},
	{Name: "time", Content: TimeLib},
}

var StdLibFiles = initStdLibFiles()

func initStdLibFiles() map[string]string {
	result := make(map[string]string)
	for _, lib := range RegisteredLibs {
		result[lib.Name] = lib.Content
	}
	return result
}

func RegisterLibrary(name string, content string) {
	RegisteredLibs = append(RegisteredLibs, Library{
		Name:    name,
		Content: content,
	})
	StdLibFiles[name] = content
}

func AutoRegisterLibraryFromFile(path string) error {
	content, err := os.ReadFile(path)
	if err != nil {
		return err
	}

	name := strings.TrimSuffix(filepath.Base(path), filepath.Ext(path))
	RegisterLibrary(name, string(content))
	return nil
}

func AutoRegisterLibrariesFromDir(dirPath string) error {
	entries, err := os.ReadDir(dirPath)
	if err != nil {
		return err
	}

	for _, entry := range entries {
		if entry.IsDir() {
			continue
		}

		if filepath.Ext(entry.Name()) == ".bn" {
			filePath := filepath.Join(dirPath, entry.Name())
			if err := AutoRegisterLibraryFromFile(filePath); err != nil {
				return err
			}
		}
	}

	return nil
}
