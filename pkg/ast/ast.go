package ast

type Node interface{}

type Expression interface {
	Node
	exprNode()
}

type Declaration interface {
	Node
	declNode()
}

type Statement interface {
	Declaration
	stmtNode()
}

type Program struct {
	Declarations []Declaration
}

type TypeDefinition struct {
	Name   string
	Fields []TypeField
}

func (t *TypeDefinition) declNode() {}

type TypeField struct {
	Name string
	Type string
}

type FunctionDeclaration struct {
	Name       string
	Parameters []Parameter
	ReturnType string
	Body       []Declaration
}

func (f *FunctionDeclaration) declNode() {}

type Parameter struct {
	Name string
	Type string
}

type VariableDeclaration struct {
	Name    string
	Type    string
	Value   Expression
	IsConst bool
}

func (v *VariableDeclaration) declNode() {}

type BlockStatement struct {
	Statements []Declaration
}

func (b *BlockStatement) declNode() {}
func (b *BlockStatement) stmtNode() {}

type ReturnStatement struct {
	Value Expression
}

func (r *ReturnStatement) declNode() {}
func (r *ReturnStatement) stmtNode() {}

type IfStatement struct {
	Condition  Expression
	ThenBranch []Declaration
	ElseBranch []Declaration
}

func (i *IfStatement) declNode() {}
func (i *IfStatement) stmtNode() {}

type WhileStatement struct {
	Condition Expression
	Body      []Declaration
}

func (w *WhileStatement) declNode() {}
func (w *WhileStatement) stmtNode() {}

type ForStatement struct {
	Initializer Declaration
	Condition   Expression
	Increment   Expression
	Body        []Declaration
}

func (f *ForStatement) declNode() {}
func (f *ForStatement) stmtNode() {}

type ExpressionStatement struct {
	Expression Expression
}

func (e *ExpressionStatement) declNode() {}
func (e *ExpressionStatement) stmtNode() {}

type BinaryExpression struct {
	Left     Expression
	Operator string
	Right    Expression
}

func (b *BinaryExpression) exprNode() {}

type UnaryExpression struct {
	Operator string
	Right    Expression
}

func (u *UnaryExpression) exprNode() {}

type CallExpression struct {
	Callee    Expression
	Arguments []Expression
}

func (c *CallExpression) exprNode() {}

type GetExpression struct {
	Object Expression
	Name   string
}

func (g *GetExpression) exprNode() {}

type SetExpression struct {
	Object Expression
	Name   string
	Value  Expression
}

func (s *SetExpression) exprNode() {}

type IndexExpression struct {
	Array Expression
	Index Expression
}

func (i *IndexExpression) exprNode() {}

type SliceExpression struct {
	Array Expression
	Start Expression
	End   Expression
}

func (s *SliceExpression) exprNode() {}

type ArrayLiteralExpression struct {
	Elements []Expression
}

func (a *ArrayLiteralExpression) exprNode() {}

type StructLiteralExpression struct {
	Type   string
	Fields map[string]Expression
}

func (s *StructLiteralExpression) exprNode() {}

type ClassMethodCallExpression struct {
	ClassName  string
	MethodName string
	Arguments  []Expression
}

func (c *ClassMethodCallExpression) exprNode() {}

type VariableExpression struct {
	Name string
}

func (v *VariableExpression) exprNode() {}

type AssignmentExpression struct {
	Name  string
	Value Expression
}

func (a *AssignmentExpression) exprNode() {}

type CompoundAssignmentExpression struct {
	Name     string
	Operator string
	Value    Expression
}

func (c *CompoundAssignmentExpression) exprNode() {}

type LiteralExpression struct {
	Value interface{}
	Type  string
	Raw   string
}

func (l *LiteralExpression) exprNode() {}

type GroupingExpression struct {
	Expression Expression
}

func (g *GroupingExpression) exprNode() {}

type LambdaExpression struct {
	Parameters []Parameter
	ReturnType string
	Body       []Declaration
}

func (l *LambdaExpression) exprNode() {}

type ThisExpression struct{}

func (t *ThisExpression) exprNode() {}

type NilExpression struct{}

func (n *NilExpression) exprNode() {}

type CastExpression struct {
	Expression Expression
	TargetType string
}

func (c *CastExpression) exprNode() {}

type RangeExpression struct {
	Start Expression
	End   Expression
	Step  Expression
}

func (r *RangeExpression) exprNode() {}

type ErrorNode struct {
	Message string
}

func (e *ErrorNode) exprNode() {}
func (e *ErrorNode) declNode() {}
func (e *ErrorNode) stmtNode() {}

type ImportDeclaration struct {
	Path string
}

func (i *ImportDeclaration) declNode() {}

type MultiImportDeclaration struct {
	Imports []*ImportDeclaration
}

func (m *MultiImportDeclaration) declNode() {}

type ClassDeclaration struct {
	Name    string
	Methods []*FunctionDeclaration
}

func (c *ClassDeclaration) declNode() {}

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
