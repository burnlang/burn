(line_comment) @comment
(block_comment) @comment

(string) @string
(escape_sequence) @string.escape
(interpolation
  "${" @punctuation.special
  "}" @punctuation.special)

(number) @number
(boolean) @boolean
(null) @constant.builtin
(self) @variable.builtin

(type_identifier) @type
(definition_kind) @keyword
(modifier) @keyword
(enum_variant) @constant

(annotation
  "@" @attribute
  name: (identifier) @attribute)

(function_declaration
  name: (identifier) @function)
(object_method
  name: (identifier) @function)
(call_expression
  function: (identifier) @function.call)
(call_expression
  function: (member_expression
    property: (property_identifier) @function.method.call))
(new_expression
  type: (type_identifier) @constructor)
(struct_literal
  type: (type_identifier) @constructor)

(parameter
  name: (identifier) @variable.parameter)
(field_declaration
  name: (identifier) @property)
(field_initializer
  name: (identifier) @property)
(member_expression
  property: (property_identifier) @property)
(named_argument
  name: (identifier) @property)

((identifier) @constant
  (#match? @constant "^[A-Z][A-Z0-9_]+$"))

[
  "fun"
  "var"
  "const"
  "def"
  "import"
  "new"
  "await"
  "is"
  "as"
  "as?"
] @keyword

[
  "if"
  "else"
  "while"
  "for"
  "in"
  "return"
  "match"
] @keyword.control

(break_statement) @keyword.control
(continue_statement) @keyword.control

[
  "="
  "+="
  "-="
  "*="
  "/="
  "%="
  "&="
  "|="
  "^="
  "<<="
  ">>="
  ">>>="
  "=="
  "!="
  "<"
  ">"
  "<="
  ">="
  "&&"
  "||"
  "!"
  "+"
  "-"
  "*"
  "/"
  "%"
  "&"
  "|"
  "^"
  "~"
  "<<"
  ">>"
  ">>>"
  "??"
  "!!"
  ".."
  "..="
  "=>"
  "->"
  "?"
] @operator

[
  "("
  ")"
  "["
  "]"
  "{"
  "}"
] @punctuation.bracket

[
  ","
  "."
  "?."
  ":"
  "::"
  ";"
] @punctuation.delimiter
