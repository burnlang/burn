package ast

type Node interface {
	Pos() int
}

type Expression interface {
	Node
	expressionNode()
}

type Declaration interface {
	Node
	declarationNode()
}

type Statement interface {
	Declaration
	stmtNode()
}

type Program struct {
	Declarations []Declaration
	Position     int
}

func (p *Program) Pos() int {
	return p.Position
}

type TypeDefinition struct {
	Name     string
	Fields   []TypeField
	Position int
}

func (t *TypeDefinition) declarationNode() {}
func (t *TypeDefinition) Pos() int {
	return t.Position
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

type BlockStatement struct {
	Statements []Declaration
	Position   int
}

func (b *BlockStatement) declarationNode() {}
func (b *BlockStatement) stmtNode()        {}
func (b *BlockStatement) Pos() int {
	return b.Position
}

type ReturnStatement struct {
	Value    Expression
	Position int
}

func (r *ReturnStatement) declarationNode() {}
func (r *ReturnStatement) stmtNode()        {}
func (r *ReturnStatement) Pos() int {
	return r.Position
}

type IfStatement struct {
	Condition  Expression
	ThenBranch []Declaration
	ElseBranch []Declaration
	Position   int
}

func (i *IfStatement) declarationNode() {}
func (i *IfStatement) stmtNode()        {}
func (i *IfStatement) Pos() int {
	return i.Position
}

type WhileStatement struct {
	Condition Expression
	Body      []Declaration
	Position  int
}

func (w *WhileStatement) declarationNode() {}
func (w *WhileStatement) stmtNode()        {}
func (w *WhileStatement) Pos() int {
	return w.Position
}

type ForStatement struct {
	Initializer Declaration
	Condition   Expression
	Increment   Expression
	Body        []Declaration
	Position    int
}

func (f *ForStatement) declarationNode() {}
func (f *ForStatement) stmtNode()        {}
func (f *ForStatement) Pos() int {
	return f.Position
}

type ExpressionStatement struct {
	Expression Expression
	Position   int
}

func (e *ExpressionStatement) declarationNode() {}
func (e *ExpressionStatement) stmtNode()        {}
func (e *ExpressionStatement) Pos() int {
	return e.Position
}

type BinaryExpression struct {
	Left     Expression
	Operator string
	Right    Expression
	Position int
}

func (b *BinaryExpression) expressionNode() {}
func (b *BinaryExpression) Pos() int {
	return b.Position
}

type UnaryExpression struct {
	Operator string
	Right    Expression
	Position int
}

func (u *UnaryExpression) expressionNode() {}
func (u *UnaryExpression) Pos() int {
	return u.Position
}

type CallExpression struct {
	Callee    Expression
	Arguments []Expression
	Position  int
}

func (c *CallExpression) expressionNode() {}
func (c *CallExpression) Pos() int {
	return c.Position
}

type GetExpression struct {
	Object   Expression
	Name     string
	Position int
}

func (g *GetExpression) expressionNode() {}
func (g *GetExpression) Pos() int {
	return g.Position
}

type SetExpression struct {
	Object   Expression
	Name     string
	Value    Expression
	Position int
}

func (s *SetExpression) expressionNode() {}
func (s *SetExpression) Pos() int {
	return s.Position
}

type IndexExpression struct {
	Array    Expression
	Index    Expression
	Position int
}

func (i *IndexExpression) expressionNode() {}
func (i *IndexExpression) Pos() int {
	return i.Position
}

type SliceExpression struct {
	Array    Expression
	Start    Expression
	End      Expression
	Position int
}

func (s *SliceExpression) expressionNode() {}
func (s *SliceExpression) Pos() int {
	return s.Position
}

type ArrayLiteralExpression struct {
	Elements []Expression
	Position int
}

func (a *ArrayLiteralExpression) expressionNode() {}
func (a *ArrayLiteralExpression) Pos() int {
	return a.Position
}

type StructLiteralExpression struct {
	Type     string
	Fields   map[string]Expression
	Position int
}

func (s *StructLiteralExpression) expressionNode() {}
func (s *StructLiteralExpression) Pos() int {
	return s.Position
}

type ClassMethodCallExpression struct {
	ClassName  string
	MethodName string
	Arguments  []Expression
	Position   int
}

func (c *ClassMethodCallExpression) expressionNode() {}
func (c *ClassMethodCallExpression) Pos() int {
	return c.Position
}

type VariableExpression struct {
	Name     string
	Position int
}

func (v *VariableExpression) expressionNode() {}
func (v *VariableExpression) Pos() int {
	return v.Position
}

type AssignmentExpression struct {
	Name     string
	Value    Expression
	Position int
}

func (a *AssignmentExpression) expressionNode() {}
func (a *AssignmentExpression) Pos() int {
	return a.Position
}

type CompoundAssignmentExpression struct {
	Name     string
	Operator string
	Value    Expression
	Position int
}

func (c *CompoundAssignmentExpression) expressionNode() {}
func (c *CompoundAssignmentExpression) Pos() int {
	return c.Position
}

type LiteralExpression struct {
	Value    interface{}
	Type     string
	Raw      string
	Position int
}

func (l *LiteralExpression) expressionNode() {}
func (l *LiteralExpression) Pos() int {
	return l.Position
}

type GroupingExpression struct {
	Expression Expression
	Position   int
}

func (g *GroupingExpression) expressionNode() {}
func (g *GroupingExpression) Pos() int {
	return g.Position
}

type LambdaExpression struct {
	Parameters []Parameter
	ReturnType string
	Body       []Declaration
	Position   int
}

func (l *LambdaExpression) expressionNode() {}
func (l *LambdaExpression) Pos() int {
	return l.Position
}

type ThisExpression struct {
	Position int
}

func (t *ThisExpression) expressionNode() {}
func (t *ThisExpression) Pos() int {
	return t.Position
}

type NilExpression struct {
	Position int
}

func (n *NilExpression) expressionNode() {}
func (n *NilExpression) Pos() int {
	return n.Position
}

type CastExpression struct {
	Expression Expression
	TargetType string
	Position   int
}

func (c *CastExpression) expressionNode() {}
func (c *CastExpression) Pos() int {
	return c.Position
}

type RangeExpression struct {
	Start    Expression
	End      Expression
	Step     Expression
	Position int
}

func (r *RangeExpression) expressionNode() {}
func (r *RangeExpression) Pos() int {
	return r.Position
}

type ErrorNode struct {
	Message  string
	Position int
}

func (e *ErrorNode) expressionNode()  {}
func (e *ErrorNode) declarationNode() {}
func (e *ErrorNode) stmtNode()        {}
func (e *ErrorNode) Pos() int {
	return e.Position
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

type Visitor interface {
	VisitProgram(program *Program) interface{}
	VisitTypeDefinition(typeDef *TypeDefinition) interface{}
	VisitFunctionDeclaration(funDecl *FunctionDeclaration) interface{}
	VisitVariableDeclaration(varDecl *VariableDeclaration) interface{}
	VisitBlockStatement(blockStmt *BlockStatement) interface{}
	VisitReturnStatement(returnStmt *ReturnStatement) interface{}
	VisitIfStatement(ifStmt *IfStatement) interface{}
	VisitWhileStatement(whileStmt *WhileStatement) interface{}
	VisitForStatement(forStmt *ForStatement) interface{}
	VisitExpressionStatement(exprStmt *ExpressionStatement) interface{}
	VisitBinaryExpression(binaryExpr *BinaryExpression) interface{}
	VisitUnaryExpression(unaryExpr *UnaryExpression) interface{}
	VisitCallExpression(callExpr *CallExpression) interface{}
	VisitGetExpression(getExpr *GetExpression) interface{}
	VisitSetExpression(setExpr *SetExpression) interface{}
	VisitIndexExpression(indexExpr *IndexExpression) interface{}
	VisitSliceExpression(sliceExpr *SliceExpression) interface{}
	VisitArrayLiteralExpression(arrayLiteral *ArrayLiteralExpression) interface{}
	VisitStructLiteralExpression(structLiteral *StructLiteralExpression) interface{}
	VisitClassMethodCallExpression(callExpr *ClassMethodCallExpression) interface{}
	VisitVariableExpression(varExpr *VariableExpression) interface{}
	VisitAssignmentExpression(assignExpr *AssignmentExpression) interface{}
	VisitCompoundAssignmentExpression(compoundExpr *CompoundAssignmentExpression) interface{}
	VisitLiteralExpression(literalExpr *LiteralExpression) interface{}
	VisitGroupingExpression(groupingExpr *GroupingExpression) interface{}
	VisitLambdaExpression(lambdaExpr *LambdaExpression) interface{}
	VisitThisExpression(thisExpr *ThisExpression) interface{}
	VisitNilExpression(nilExpr *NilExpression) interface{}
	VisitCastExpression(castExpr *CastExpression) interface{}
	VisitRangeExpression(rangeExpr *RangeExpression) interface{}
	VisitErrorNode(errorNode *ErrorNode) interface{}
}

func (p *Program) Accept(visitor Visitor) interface{} {
	return visitor.VisitProgram(p)
}

func (t *TypeDefinition) Accept(visitor Visitor) interface{} {
	return visitor.VisitTypeDefinition(t)
}

func (f *FunctionDeclaration) Accept(visitor Visitor) interface{} {
	return visitor.VisitFunctionDeclaration(f)
}

func (v *VariableDeclaration) Accept(visitor Visitor) interface{} {
	return visitor.VisitVariableDeclaration(v)
}

func (b *BlockStatement) Accept(visitor Visitor) interface{} {
	return visitor.VisitBlockStatement(b)
}

func (r *ReturnStatement) Accept(visitor Visitor) interface{} {
	return visitor.VisitReturnStatement(r)
}

func (i *IfStatement) Accept(visitor Visitor) interface{} {
	return visitor.VisitIfStatement(i)
}

func (w *WhileStatement) Accept(visitor Visitor) interface{} {
	return visitor.VisitWhileStatement(w)
}

func (f *ForStatement) Accept(visitor Visitor) interface{} {
	return visitor.VisitForStatement(f)
}

func (e *ExpressionStatement) Accept(visitor Visitor) interface{} {
	return visitor.VisitExpressionStatement(e)
}

func (b *BinaryExpression) Accept(visitor Visitor) interface{} {
	return visitor.VisitBinaryExpression(b)
}

func (u *UnaryExpression) Accept(visitor Visitor) interface{} {
	return visitor.VisitUnaryExpression(u)
}

func (c *CallExpression) Accept(visitor Visitor) interface{} {
	return visitor.VisitCallExpression(c)
}

func (g *GetExpression) Accept(visitor Visitor) interface{} {
	return visitor.VisitGetExpression(g)
}

func (s *SetExpression) Accept(visitor Visitor) interface{} {
	return visitor.VisitSetExpression(s)
}

func (i *IndexExpression) Accept(visitor Visitor) interface{} {
	return visitor.VisitIndexExpression(i)
}

func (s *SliceExpression) Accept(visitor Visitor) interface{} {
	return visitor.VisitSliceExpression(s)
}

func (a *ArrayLiteralExpression) Accept(visitor Visitor) interface{} {
	return visitor.VisitArrayLiteralExpression(a)
}

func (s *StructLiteralExpression) Accept(visitor Visitor) interface{} {
	return visitor.VisitStructLiteralExpression(s)
}

func (c *ClassMethodCallExpression) Accept(visitor Visitor) interface{} {
	return visitor.VisitClassMethodCallExpression(c)
}

func (v *VariableExpression) Accept(visitor Visitor) interface{} {
	return visitor.VisitVariableExpression(v)
}

func (a *AssignmentExpression) Accept(visitor Visitor) interface{} {
	return visitor.VisitAssignmentExpression(a)
}

func (c *CompoundAssignmentExpression) Accept(visitor Visitor) interface{} {
	return visitor.VisitCompoundAssignmentExpression(c)
}

func (l *LiteralExpression) Accept(visitor Visitor) interface{} {
	return visitor.VisitLiteralExpression(l)
}

func (g *GroupingExpression) Accept(visitor Visitor) interface{} {
	return visitor.VisitGroupingExpression(g)
}

func (l *LambdaExpression) Accept(visitor Visitor) interface{} {
	return visitor.VisitLambdaExpression(l)
}

func (t *ThisExpression) Accept(visitor Visitor) interface{} {
	return visitor.VisitThisExpression(t)
}

func (n *NilExpression) Accept(visitor Visitor) interface{} {
	return visitor.VisitNilExpression(n)
}

func (c *CastExpression) Accept(visitor Visitor) interface{} {
	return visitor.VisitCastExpression(c)
}

func (r *RangeExpression) Accept(visitor Visitor) interface{} {
	return visitor.VisitRangeExpression(r)
}

func (e *ErrorNode) Accept(visitor Visitor) interface{} {
	return visitor.VisitErrorNode(e)
}

func (p *Program) String() string {
	return "Program"
}

func (t *TypeDefinition) String() string {
	return "TypeDefinition: " + t.Name
}

func (f *FunctionDeclaration) String() string {
	return "FunctionDeclaration: " + f.Name
}

func (v *VariableDeclaration) String() string {
	constStr := ""
	if v.IsConst {
		constStr = " (const)"
	}
	return "VariableDeclaration: " + v.Name + constStr
}

func (b *BlockStatement) String() string {
	return "BlockStatement"
}

func (r *ReturnStatement) String() string {
	return "ReturnStatement"
}

func (i *IfStatement) String() string {
	return "IfStatement"
}

func (w *WhileStatement) String() string {
	return "WhileStatement"
}

func (f *ForStatement) String() string {
	return "ForStatement"
}

func (e *ExpressionStatement) String() string {
	return "ExpressionStatement"
}

func (b *BinaryExpression) String() string {
	return "BinaryExpression: " + b.Operator
}

func (u *UnaryExpression) String() string {
	return "UnaryExpression: " + u.Operator
}

func (c *CallExpression) String() string {
	return "CallExpression"
}

func (g *GetExpression) String() string {
	return "GetExpression: ." + g.Name
}

func (s *SetExpression) String() string {
	return "SetExpression: ." + s.Name
}

func (i *IndexExpression) String() string {
	return "IndexExpression"
}

func (s *SliceExpression) String() string {
	return "SliceExpression"
}

func (a *ArrayLiteralExpression) String() string {
	return "ArrayLiteralExpression"
}

func (s *StructLiteralExpression) String() string {
	return "StructLiteralExpression"
}

func (c *ClassMethodCallExpression) String() string {
	return "ClassMethodCallExpression: " + c.ClassName + "." + c.MethodName
}

func (v *VariableExpression) String() string {
	return "VariableExpression: " + v.Name
}

func (a *AssignmentExpression) String() string {
	return "AssignmentExpression: " + a.Name
}

func (c *CompoundAssignmentExpression) String() string {
	return "CompoundAssignmentExpression: " + c.Name + " " + c.Operator
}

func (l *LiteralExpression) String() string {
	return "LiteralExpression: " + l.Raw
}

func (g *GroupingExpression) String() string {
	return "GroupingExpression"
}

func (l *LambdaExpression) String() string {
	return "LambdaExpression"
}

func (t *ThisExpression) String() string {
	return "ThisExpression"
}

func (n *NilExpression) String() string {
	return "NilExpression"
}

func (c *CastExpression) String() string {
	return "CastExpression: to " + c.TargetType
}

func (r *RangeExpression) String() string {
	return "RangeExpression"
}

func (e *ErrorNode) String() string {
	return "ErrorNode: " + e.Message
}
