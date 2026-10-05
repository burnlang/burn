const PREC = {
  assign: 1,
  coalesce: 2,
  or: 3,
  and: 4,
  bit_or: 5,
  bit_xor: 6,
  bit_and: 7,
  equality: 8,
  compare: 9,
  is: 10,
  range: 11,
  shift: 12,
  add: 13,
  mul: 14,
  unary: 15,
  postfix: 16,
  call: 17,
};

const commaSep = (rule) => optional(seq(rule, repeat(seq(',', rule)), optional(',')));
const sepBy = (sep, rule) => optional(seq(rule, repeat(seq(sep, rule)), optional(sep)));

module.exports = grammar({
  name: 'burn',

  extras: ($) => [/\s/, $.line_comment, $.block_comment],

  word: ($) => $.identifier,

  conflicts: ($) => [
    [$.object_method, $._expression],
    [$.map_literal, $.match_expression],
    [$._record_definition],
    [$._struct_definition],
    [$.parameter, $._type_name],
    [$.parameters, $.function_type],
    [$.modifier, $.lambda],
    [$._type_name, $._expression],
    [$.block, $.map_literal],
  ],

  supertypes: ($) => [$._expression, $._statement],

  rules: {
    source_file: ($) => repeat(choice($._item, ';')),

    _item: ($) => choice($.import_declaration, $.function_declaration, $.definition, $._statement),

    line_comment: (_) => token(seq('//', /[^\n]*/)),
    block_comment: (_) => token(seq('/*', /[^*]*\*+([^/*][^*]*\*+)*/, '/')),

    import_declaration: ($) => seq('import', choice($.string, seq('(', repeat(seq($.string, optional(','))), ')'))),

    annotation: ($) => prec.right(seq('@', field('name', $.identifier), optional($.arguments))),

    modifier: (_) => choice('pub', 'priv', 'async', 'static', 'abstract'),
    _modifiers: ($) => repeat1($.modifier),
    _annotations: ($) => repeat1($.annotation),

    function_declaration: ($) =>
      prec.right(
        seq(
          optional($._annotations),
          optional($._modifiers),
          'fun',
          field('name', $.identifier),
          optional($.type_parameters),
          field('parameters', $.parameters),
          optional(seq(choice(':', '->'), field('return_type', $._type))),
          optional(field('body', $.block)),
        ),
      ),

    type_parameters: ($) => seq('<', commaSep($.identifier), '>'),

    parameters: ($) => seq('(', commaSep($.parameter), ')'),
    parameter: ($) =>
      seq(
        optional($._annotations),
        choice(
          seq(field('name', $.identifier), optional(seq(':', field('type', $._type), optional('...')))),
          seq(field('type', $._type), field('name', $.identifier)),
        ),
        optional(seq('=', field('default', $._expression))),
      ),

    definition: ($) =>
      seq(
        optional($._annotations),
        optional($._modifiers),
        'def',
        choice($._struct_definition, $._record_definition, $._interface_definition, $._enum_definition),
      ),

    _struct_definition: ($) =>
      seq(
        repeat(alias(choice('abstract', 'static'), $.modifier)),
        alias(choice('struct', 'class'), $.definition_kind),
        field('name', $.type_identifier),
        optional($.type_parameters),
        optional(field('parameters', $.parameters)),
        optional(seq(':', field('parent', $._type), optional($.arguments))),
        optional(seq('::', commaSep(field('implements', $._type)))),
        optional(field('body', $.struct_body)),
      ),

    struct_body: ($) => prec.dynamic(3, seq('{', repeat(choice($._member, ';', ',')), '}')),

    _member: ($) => choice($.function_declaration, $.field_declaration, $._statement),

    field_declaration: ($) =>
      prec.right(
        1,
        seq(
          optional($._annotations),
          optional($._modifiers),
          choice(
            seq(field('name', $.identifier), ':', field('type', $._type)),
            seq(field('type', $._type), field('name', $.identifier)),
            seq(choice('var', 'const'), field('name', $.identifier), optional(seq(':', field('type', $._type)))),
          ),
          optional(seq('=', field('value', $._expression))),
        ),
      ),

    _record_definition: ($) =>
      seq(
        alias(choice('type', 'record', 'annotation'), $.definition_kind),
        field('name', $.type_identifier),
        optional($.type_parameters),
        optional(choice(seq('=', field('value', $._type)), field('body', $.record_body))),
      ),

    record_body: ($) => prec.dynamic(3, seq('{', repeat(choice($.field_declaration, ',', ';')), '}')),

    _interface_definition: ($) =>
      seq(
        alias(choice('interface', 'trait'), $.definition_kind),
        field('name', $.type_identifier),
        optional($.type_parameters),
        field('body', $.interface_body),
      ),

    interface_body: ($) => prec.dynamic(3, seq('{', repeat(choice($.function_declaration, ',', ';')), '}')),

    _enum_definition: ($) =>
      seq(alias('enum', $.definition_kind), field('name', $.type_identifier), field('body', $.enum_body)),

    enum_body: ($) => prec.dynamic(3, seq('{', sepBy(optional(choice(',', ';')), $.enum_variant), '}')),
    enum_variant: ($) => $.identifier,

    _type: ($) => choice($._type_name, $.generic_type, $.array_type, $.map_type, $.optional_type, $.function_type, $.parenthesized_type),
    _type_name: ($) => alias($.identifier, $.type_identifier),
    type_identifier: ($) => alias($.identifier, 'type_identifier'),
    generic_type: ($) => prec(1, seq($._type_name, '<', commaSep($._type), '>')),
    array_type: ($) => seq('[', $._type, ']'),
    map_type: ($) => seq('{', $._type, ':', $._type, '}'),
    optional_type: ($) => prec(2, seq($._type, '?')),
    function_type: ($) => prec.right(seq('fun', '(', commaSep($._type), ')', optional(seq(':', $._type)))),
    parenthesized_type: ($) => seq('(', $._type, ')'),

    _statement: ($) =>
      choice(
        $.variable_declaration,
        $.typed_declaration,
        $.if_statement,
        $.while_statement,
        $.for_statement,
        $.return_statement,
        $.break_statement,
        $.continue_statement,
        $.block,
        $.object_method,
        $._expression,
      ),

    object_method: ($) =>
      prec.dynamic(
        1,
        seq(
          field('object', $.identifier),
          '.',
          field('name', $.identifier),
          field('parameters', $.parameters),
          optional(seq(choice(':', '->'), field('return_type', $._type))),
          field('body', $.block),
        ),
      ),

    variable_declaration: ($) =>
      prec.right(
        seq(
          optional($._annotations),
          optional($._modifiers),
          choice('var', 'const'),
          field('name', $.identifier),
          optional(seq(':', field('type', $._type))),
          optional(seq('=', field('value', $._expression))),
        ),
      ),

    typed_declaration: ($) =>
      prec.dynamic(
        2,
        prec.right(
          seq(field('type', $._type), field('name', $.identifier), optional(seq('=', field('value', $._expression)))),
        ),
      ),

    if_statement: ($) =>
      prec.right(
        seq(
          'if',
          field('condition', $._expression),
          field('consequence', $.block),
          optional(seq('else', field('alternative', choice($.if_statement, $.block)))),
        ),
      ),

    while_statement: ($) => seq('while', field('condition', $._expression), field('body', $.block)),

    for_statement: ($) =>
      seq(
        'for',
        choice(
          seq(
            '(',
            optional(choice($.variable_declaration, $.typed_declaration, $._expression)),
            ';',
            optional($._expression),
            ';',
            optional($._expression),
            ')',
          ),
          seq('(', $._for_header, ')'),
          $._for_header,
        ),
        field('body', $.block),
      ),

    _for_header: ($) =>
      seq(field('name', $.identifier), optional(seq(',', field('value', $.identifier))), 'in', field('iterable', $._expression)),

    return_statement: ($) => prec.right(seq('return', optional($._expression))),
    break_statement: (_) => 'break',
    continue_statement: (_) => 'continue',

    block: ($) => prec.dynamic(1, seq('{', repeat(choice($._statement, $.function_declaration, ';')), '}')),

    _expression: ($) =>
      choice(
        $.identifier,
        $.self,
        $.number,
        $.string,
        $.boolean,
        $.null,
        $.array_literal,
        $.map_literal,
        $.struct_literal,
        $.parenthesized_expression,
        $.unary_expression,
        $.binary_expression,
        $.assignment_expression,
        $.call_expression,
        $.member_expression,
        $.index_expression,
        $.non_null_expression,
        $.cast_expression,
        $.is_expression,
        $.new_expression,
        $.lambda,
        $.match_expression,
        $.await_expression,
        $.range_expression,
      ),

    self: (_) => 'self',
    boolean: (_) => choice('true', 'false'),
    null: (_) => 'null',

    number: (_) =>
      token(choice(/0[xX][0-9a-fA-F_]+/, /0[bB][01_]+/, /0[oO][0-7_]+/, /\d[\d_]*(\.\d[\d_]*)?([eE][+-]?\d+)?/)),

    string: ($) =>
      choice(
        seq('"', repeat(choice($.string_content, $.escape_sequence, $.interpolation, alias('$', $.string_content))), '"'),
        seq("'", repeat(choice(alias($._single_content, $.string_content), $.escape_sequence, $.interpolation, alias('$', $.string_content))), "'"),
      ),
    string_content: (_) => token.immediate(prec(1, /[^"\\$]+/)),
    _single_content: (_) => token.immediate(prec(1, /[^'\\$]+/)),
    escape_sequence: (_) => token.immediate(seq('\\', choice(/u\{[0-9a-fA-F]+\}/, /./))),
    interpolation: ($) => seq(token.immediate('${'), $._expression, '}'),

    array_literal: ($) => seq('[', commaSep($._expression), ']'),

    map_literal: ($) => seq('{', commaSep($.map_entry), '}'),
    map_entry: ($) => seq(field('key', $._expression), ':', field('value', $._expression)),

    struct_literal: ($) =>
      prec.dynamic(2, prec(1, seq(field('type', choice($._type_name, $.generic_type)), '{', commaSep($.field_initializer), '}'))),
    field_initializer: ($) => seq(field('name', $.identifier), ':', field('value', $._expression)),

    parenthesized_expression: ($) => seq('(', $._expression, ')'),

    unary_expression: ($) => prec(PREC.unary, seq(field('operator', choice('-', '!', '~')), field('argument', $._expression))),
    await_expression: ($) => prec(PREC.unary, seq('await', $._expression)),

    binary_expression: ($) => {
      const table = [
        [PREC.coalesce, '??'],
        [PREC.or, '||'],
        [PREC.and, '&&'],
        [PREC.bit_or, '|'],
        [PREC.bit_xor, '^'],
        [PREC.bit_and, '&'],
        [PREC.equality, choice('==', '!=')],
        [PREC.compare, choice('<', '>', '<=', '>=')],
        [PREC.shift, choice('<<', '>>', '>>>')],
        [PREC.add, choice('+', '-')],
        [PREC.mul, choice('*', '/', '%')],
      ];
      return choice(
        ...table.map(([p, op]) =>
          prec.left(p, seq(field('left', $._expression), field('operator', op), field('right', $._expression))),
        ),
      );
    },

    range_expression: ($) => prec.left(PREC.range, seq($._expression, choice('..', '..='), $._expression)),

    assignment_expression: ($) =>
      prec.right(
        PREC.assign,
        seq(
          field('left', $._expression),
          field('operator', choice('=', '+=', '-=', '*=', '/=', '%=', '&=', '|=', '^=', '<<=', '>>=', '>>>=')),
          field('right', $._expression),
        ),
      ),

    cast_expression: ($) =>
      prec.left(
        PREC.is,
        seq($._expression, choice(alias(token.immediate(prec(2, /[ \t]*as/)), 'as'), alias(token.immediate(prec(3, /[ \t]*as\?/)), 'as?')), $._type),
      ),
    is_expression: ($) => prec.left(PREC.is, seq($._expression, alias(token.immediate(prec(2, /[ \t]*is/)), 'is'), $._type)),

    call_expression: ($) =>
      prec(PREC.call, seq(field('function', $._expression), field('arguments', alias($.call_arguments, $.arguments)))),
    call_arguments: ($) => seq(alias(token.immediate(prec(2, /[ \t]*\(/)), '('), commaSep(choice($._expression, $.named_argument)), ')'),
    arguments: ($) => seq('(', commaSep(choice($._expression, $.named_argument)), ')'),
    named_argument: ($) => seq(field('name', $.identifier), ':', field('value', $._expression)),

    member_expression: ($) =>
      prec(PREC.postfix, seq(field('object', $._expression), choice('.', '?.'), field('property', alias($.identifier, $.property_identifier)))),

    index_expression: ($) =>
      prec(PREC.postfix, seq(field('object', $._expression), alias(token.immediate(prec(2, /[ \t]*\[/)), '['), field('index', $._expression), ']')),

    non_null_expression: ($) => prec(PREC.postfix, seq($._expression, '!!')),

    new_expression: ($) =>
      prec(PREC.call, seq('new', field('type', choice($._type_name, $.generic_type)), field('arguments', $.arguments))),

    lambda: ($) =>
      seq(
        optional('async'),
        'fun',
        field('parameters', $.parameters),
        optional(seq(choice(':', '->'), field('return_type', $._type))),
        field('body', $.block),
      ),

    match_expression: ($) => prec.right(seq('match', optional(field('subject', $._expression)), '{', repeat(choice($.match_arm, ',', ';')), '}')),
    match_arm: ($) =>
      seq(
        field('pattern', choice('else', $.match_pattern)),
        optional(seq('if', field('guard', $._expression))),
        '=>',
        field('value', choice($.block, $._expression)),
      ),
    match_pattern: ($) => seq($._pattern, repeat(seq(',', $._pattern))),
    _pattern: ($) => choice($._expression, seq('is', $._type, optional($.identifier))),

    identifier: (_) => /[A-Za-z_\u00C0-\uFFFF][A-Za-z0-9_\u00C0-\uFFFF]*/,
  },
});
