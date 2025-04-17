package ast

type TypeDefinition struct {
	Name     string
	Fields   []TypeField
	Position int
}

func (t *TypeDefinition) declarationNode() {}
func (t *TypeDefinition) Pos() int {
	return t.Position
}

func (t *TypeDefinition) String() string {
	return "TypeDefinition: " + t.Name
}

type TypeField struct {
	Name     string
	Type     string
	Position int
}

func (t *TypeField) Pos() int {
	return t.Position
}

type FunctionDeclaration struct {
	Name       string
	Parameters []Parameter
	ReturnType string
	Body       []Declaration
	Position   int
}

func (f *FunctionDeclaration) declarationNode() {}
func (f *FunctionDeclaration) Pos() int {
	return f.Position
}

func (f *FunctionDeclaration) String() string {
	return "FunctionDeclaration: " + f.Name
}

type Parameter struct {
	Name     string
	Type     string
	Position int
}

func (p *Parameter) Pos() int {
	return p.Position
}

type VariableDeclaration struct {
	Name     string
	Type     string
	Value    Expression
	IsConst  bool
	Position int
}

func (v *VariableDeclaration) declarationNode() {}
func (v *VariableDeclaration) Pos() int {
	return v.Position
}

func (v *VariableDeclaration) String() string {
	constStr := ""
	if v.IsConst {
		constStr = " (const)"
	}
	return "VariableDeclaration: " + v.Name + constStr
}

type ImportDeclaration struct {
	Path     string
	Position int
}

func (i *ImportDeclaration) declarationNode() {}
func (i *ImportDeclaration) Pos() int {
	return i.Position
}

type MultiImportDeclaration struct {
	Imports  []*ImportDeclaration
	Position int
}

func (m *MultiImportDeclaration) declarationNode() {}
func (m *MultiImportDeclaration) Pos() int {
	return m.Position
}

type ClassDeclaration struct {
	Name          string
	Methods       []*FunctionDeclaration
	StaticMethods []*FunctionDeclaration
	Position      int
}

func (c *ClassDeclaration) declarationNode() {}
func (c *ClassDeclaration) Pos() int {
	return c.Position
}

func (c *ClassDeclaration) String() string {
	return "ClassDeclaration: " + c.Name
}
