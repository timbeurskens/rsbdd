grammar TPTP;
WS : [ \r\t\n]+ -> skip ;
Comment_line : '%' ~[\r\n]* -> skip;
Comment_block : '/*' .*? '*/' -> skip;
//# HERE ARE THE LEXER RULES

Single_quoted :  Single_quote   Sq_char   Sq_char * Single_quote ;
Back_quoted :  Back_quote   Upper_word ;
Distinct_object :  Double_quote   Do_char   Do_char * Double_quote ;
Dollar_word :  Dollar   Alpha_numeric *;
Dollar_dollar_word :  Dollar   Dollar   Alpha_numeric *;
Upper_word :  Upper_alpha   Alpha_numeric *;
Lower_word :  Lower_alpha   Alpha_numeric *;
// tptp2rsbdd: the parts of numbers below are fragments, so that an integer such as 1 is an Integer, not an
// Exp_integer or Integer_digits, which come first and match the same text
Real : ( Signed_real | Unsigned_real );
fragment Signed_real :  Sign   Unsigned_real ;
fragment Unsigned_real : ( Decimal_fraction | Decimal_exponent );
fragment Decimal_exponent : ( Integer_digits | Decimal_fraction ) Exponent   Exp_integer ;
fragment Decimal_fraction :  Unsigned_integer   Dot   Integer_digits ;
fragment Exp_integer : ( Signed_exp_integer | Integer_digits );
fragment Signed_exp_integer :  Sign   Integer_digits ;
Rational : ( Signed_rational | Unsigned_rational );
fragment Signed_rational :  Sign   Unsigned_rational ;
fragment Unsigned_rational :  Unsigned_integer   Slash   Positive_integer ;
Integer : ( Signed_integer | Unsigned_integer );
fragment Signed_integer :  Sign   Unsigned_integer ;
fragment Unsigned_integer : ( Zero_numeric | Positive_integer );
fragment Positive_integer :  Non_zero_numeric   Numeric *;
fragment Integer_digits :  Numeric   Numeric *;
Slash :  Slash_char ;
Slosh :  Slosh_char ;
Vline : [|];
Star : [*];
Plus : '+';
Arrow : '>';
Less_sign : '<';
Hash : '#';
///
///
///
Not_star_slash : (~'*')* '**' ~('/' | '*')*;
Percentage_sign : [%];
Double_quote : ["];
fragment Do_char : [\u0020-\u0021\u0023-\u005B\u005D-\u007E] | '\\'["\\];
Single_quote : '\'';
Back_quote : [`];
fragment Sq_char : [\u0020-\u0026\u0028-\u005B\u005D-\u007E] | '\\\\' | '\\\'';
fragment Sign : [+-];
Dot : [.];
fragment Exponent : [Ee];
Slash_char : [/];
Slosh_char : '\\\\';
Zero_numeric : [0];
fragment Non_zero_numeric : [1-9];
fragment Numeric : [0-9];
fragment Lower_alpha : [a-z];
fragment Upper_alpha : [A-Z];
Underscore : [_];
Alpha : ( Lower_alpha | Upper_alpha );
fragment Alpha_numeric : Lower_alpha | Upper_alpha | Numeric | '_';
Dollar : [$];
Comment_char : [^\r\n];

//# END THE LEXER RULES


//%-------------------------------------------------------------------------------------------------- 
//% v9.3.1.1 - Damn, to make ITV work I have to allow <functor>(<thf_arguments>), but that's not for  
//%            THF logic. Thus now it's ... 
//%            <thf_fof_function>     ::= <defined_functor>(<thf_arguments>) |  
//%                                       <system_functor>(<thf_arguments>) | 
//%                                       <functor>(<thf_arguments>) 
//% v9.3.1.2 - Fixed typos in comments (thanks to Jiazhen) 
//%          - Changed { and } to < and > in ... 
//%            <comment_block>        ::: <Slash_char> <Star> <Not_star_slash> <Star> <Star>*<Slash_char> 
//% v9.3.1.3 - Added logic to <formula_role> semantic rule. 
//%          - Completed the <ntf_modal_system> list: 
//%            $modal_system_K | $modal_system_KB | $modal_system_K4 | 
//%            $modal_system_K5 | $modal_system_K45 | $modal_system_KB5 | 
//%            $modal_system_D | $modal_system_DB | $modal_system_D4 | 
//%            $modal_system_D5 | $modal_system_D45 | $modal_system_M | 
//%            $modal_system_B | $modal_system_S4 | $modal_system_S5 | 
//%            $modal_system_S5U 
//%          - Completed the <ntf_modal_axiom> list: 
//%            $modal_axiom_K | $modal_axiom_M | $modal_axiom_B | $modal_axiom_D | 
//%            $modal_axiom_4 | $modal_axiom_5 | $modal_axiom_CD | $modal_axiom_BoxM | 
//%            $modal_axiom_C4 | $modal_axiom_C 
//%          - Added $abs to <defined_functor> semantic rule. 
//%          - Fixed <Distinct_object> to require at least one <Do_char> ... 
//%            <Double_quote> <Do_char> <Do_char>*<Double_quote> 
//%          = Removed "unknown" from <source> because it conflicts with <dag_source> 
//%          - Thanks to Johannes Schuster for finding the flaws! 
//% v9.3.1.4 - Changed <*_binary_assoc> to <*_binary_list> and <*_binary_nonassoc> to 
//%            <*_binary_pair>, to reflect the intended non-associativity. 
//% v9.3.1.5 - Changed <printable_char> ::: . to <Comment_char> ::: [^\r\n], and replaced 
//%            <printable_char> with <Comment_char> in all uses. Removed unused <viewable_char>. 
//%            Thanks to Jiazhen Xia for these fixes. 
//% v9.3.1.6 - Removed $temporal_instant and added $intuitionistic as a <ntf_logic_name> 
//%-------------------------------------------------------------------------------------------------- 
//%----README ... this header provides important meta- and usage information 
//%---- 
//%----Intended uses of the various parts of the TPTP syntax are explained in the TPTP technical 
//%----manual, linked from www.tptp.org. 
//%---- 
//%----Four kinds of separators are used, to indicate different types of rules: 
//%----  ::= is used for regular grammar rules, for syntactic parsing. 
//%----  :== is used for semantic grammar rules. These define specific values that make semantic 
//%----      sense when more general syntactic rules apply. 
//%----  ::- is used for rules that produce tokens. 
//%----  ::: is used for rules that define character classes used in the construction of tokens. 
//%---- 
//%----White space may occur between any two tokens. White space is not specified in the grammar, but 
//%----there are some restrictions to ensure that the grammar is compatible with standard Prolog: a 
//%----<TPTP_file> should be readable with read/1. 
//%---- 
//%----The syntax of comments is defined by the <comment> rule. Comments may occur between any two 
//%----tokens, but do not act as white space. Comments will normally be discarded at the lexical 
//%----level, but may be processed by systems that understand them (e.g., if the system comment 
//%----convention is followed). 
//%---- 
//%----Multiple languages are defined. Depending on your need, you can implement just the one(s) you 
//%----need. The common rules for atoms, terms, etc, come after the definitions of the languages, and 
//%----mostly all needed for all the languages. 
//%-------------------------------------------------------------------------------------------------- 
//%----Files. Empty file is OK. 
tptp_file               : tptp_input* EOF;
tptp_input              : annotated_formula | include;
//%----Formula records 
annotated_formula : thf_annotated  |  tff_annotated  |  tcf_annotated  |  fof_annotated  |  cnf_annotated  |  tpi_annotated;
//%----Future languages may include ...  english | efof | tfof | mathml | ... 
tpi_annotated : 'tpi('name','formula_role','tpi_formula annotations').';
tpi_formula : fof_formula;
thf_annotated : 'thf('name','formula_role','thf_formula annotations').';
tff_annotated : 'tff('name','formula_role','tff_formula annotations').';
tcf_annotated : 'tcf('name','formula_role','tcf_formula annotations').';
fof_annotated : 'fof('name','formula_role','fof_formula annotations').';
cnf_annotated : 'cnf('name','formula_role','cnf_formula annotations').';
annotations : ','source optional_info  |  nothing;
//%----In derivations the annotated formulae names must be unique, so that parent references (see 
//%----<inference_record>) are unambiguous. 
//%----Types for problems. 
//%----Note: The previous <source_type> from ... 
//%----   <formula_role> ::= <user_role>-<source> 
//%----... is now gone. Parsers may choose to be tolerant of it for backwards compatibility. 
formula_role : Lower_word  |  Lower_word'-'general_term;
//<formula_role>         :== axiom | hypothesis | definition | assumption | lemma | theorem | corollary | conjecture | negated_conjecture | plain | type | interpretation | logic | unknown 
//%----"axiom"s are accepted, without proof. There is no guarantee that the axioms of a problem are 
//%----consistent. "hypothesis"s are assumed to be true for a particular problem, and are used like 
//%----"axiom"s. "definition"s are intended to define symbols. They are either universally quantified 
//%----equations, or universally quantified equivalences with an atomic lefthand side. They can be 
//%----treated like "axiom"s. "assumption"s can be used like axioms, but must be discharged before a 
//%----derivation is complete. "lemma"s and "theorem"s have been proven from the "axiom"s. They can 
//%----be used like "axiom"s in problems, and a problem containing a non-redundant "lemma" or 
//%----"theorem" is ill-formed. They can also appear in derivations. "theorem"s are more important 
//%----than "lemma"s from the user perspective. "conjecture"s are to be proven from the 
//%----"axiom"(-like) formulae. A problem is solved only when all "conjecture"s are proven. 
//%----"negated_conjecture"s are formed from negation of a "conjecture" (usually in a FOF to CNF 
//%----conversion). "plain"s have no specified user semantics. "interpretation"s record all aspects 
//%----of an interpretation. "type"s defines the type globally for one symbol. "unknown"s have  
//%----unknown role, and this is an error situation. The <general_term> subroles are used in various  
//%----ways, including but not limited to: "domains" and "mappings" for "interpretation"s;  
//%----"datatype", codatatype", "datatype_constructor", and "codatatype_constructor" for "type"s. 
//%-------------------------------------------------------------------------------------------------- 
//%----THF formulae. 
thf_formula : thf_logic_formula  |  thf_atom_typing  |  thf_subtype;
thf_logic_formula : thf_unitary_formula  |  thf_unary_formula  |  thf_binary_formula  |  thf_defined_infix  |  thf_definition  |  thf_sequent;
thf_binary_formula : thf_binary_pair  |  thf_binary_list  |  thf_binary_type;
thf_binary_pair : thf_unit_formula nonassoc_connective thf_unit_formula;
//%----There's no intended precedence among associative connectives 
thf_binary_list : thf_or_formula  |  thf_and_formula  |  thf_apply_formula;
thf_or_formula : thf_unit_formula Vline thf_unit_formula  |  thf_or_formula Vline thf_unit_formula;
thf_and_formula : thf_unit_formula '&' thf_unit_formula  |  thf_and_formula '&' thf_unit_formula;
//%----@ for THF applicative style is left-associative and lambda is right-associative. 
//%----^ [X] : ^ [Y] : f @ g (where f is a <thf_apply_formula> and g is a <thf_unitary_formula>) 
//%----should be parsed as: (^ [X] : (^ [Y] : f)) @ g. That is, g is not in the scope of either 
//%----lambda. 
thf_apply_formula : thf_unit_formula '@' thf_unit_formula  |  thf_apply_formula '@' thf_unit_formula;
thf_unit_formula : thf_unitary_formula  |  thf_unary_formula  |  thf_defined_infix;
thf_preunit_formula : thf_unitary_formula  |  thf_prefix_unary;
thf_unitary_formula : thf_quantified_formula  |  thf_atomic_formula  |  variable  |  '('thf_logic_formula')';
//%----All variables must be quantified 
thf_quantified_formula : thf_quantification thf_unit_formula;
thf_quantification : thf_quantifier '['thf_variable_list']' ':';
thf_variable_list : thf_typed_variable  |  thf_typed_variable','thf_variable_list;
thf_typed_variable : variable ':' thf_top_level_type;
thf_unary_formula : thf_prefix_unary  |  thf_infix_unary;
thf_prefix_unary : thf_unary_connective thf_preunit_formula;
thf_infix_unary : thf_unitary_term infix_inequality thf_unitary_term;
thf_atomic_formula : thf_plain_atomic  |  thf_defined_atomic  |  thf_system_atomic  |  thf_fof_function;
thf_plain_atomic : constant  |  thf_tuple;
//%----<thf_plain_atomic> includes <thf_tuple> because tuples can be formulae in logic definitions 
thf_defined_atomic : defined_constant  |  thf_defined_term  |  '('thf_conn_term')'  |  nhf_long_connective  |  thf_let;
//% <thf_conditional> 
//%----<thf_conditional> is omitted from <thf_defined_atomic> because $ite is 
//%----read simply as a <thf_apply_formula> 
thf_defined_term : defined_term  |  th1_defined_term;
//%----The ()s are really optional. I have reformated the TPTP to include them so, e.g., 
//%----! [X:foo] a = X is formatted as ! [X:foo] (a = X) to save the lives of parsers that would 
//%----parse it as (! [X:foo] a) = X and throw an error. 
thf_defined_infix : thf_unitary_term defined_infix_pred thf_unitary_term;
//%----Defined terms can't be formulae. See TFF. FIX HERE. 
thf_system_atomic : system_constant;
//%----<thf_conditional> is written and read as a <thf_apply_formula> 
//% <thf_conditional>      ::= $ite(<thf_logic_formula>,<thf_logic_formula>, <thf_logic_formula>) 
thf_let : '$let('thf_let_types','thf_let_defns',' thf_logic_formula')';
thf_let_types : thf_atom_typing  |  '['thf_atom_typing_list']';
thf_atom_typing_list : thf_atom_typing  |  thf_atom_typing','thf_atom_typing_list;
thf_let_defns : thf_let_defn  |  '['thf_let_defn_list']';
thf_let_defn : thf_logic_formula assignment thf_logic_formula;
thf_let_defn_list : thf_let_defn  |  thf_let_defn','thf_let_defn_list;
thf_unitary_term : thf_atomic_formula  |  variable  |  '('thf_logic_formula')';
thf_conn_term : nonassoc_connective  |  assoc_connective  |  infix_equality  |  infix_inequality  |  thf_unary_connective;
//%----Note that syntactically this allows (p @ =), but for = the first argument must be known to 
//%----infer the type of =, so that's not allowed, i.e., only (= @ p). 
thf_tuple : '[]'  |  '['thf_formula_list']';
//%----Allows first-order functional style in THF for $words, e.g., $distinct. Damn, to make ITV 
//%----work I have to allow <functor>(<thf_arguments>), but that's not for THF logic. 
thf_fof_function : defined_functor'('thf_arguments')'  |  system_functor'('thf_arguments')'  |   functor'('thf_arguments')';
//%----Arguments recurse back up to formulae (this is the THF world here) 
thf_arguments : thf_formula_list;
thf_formula_list : thf_logic_formula comma_thf_logic_formula*;
comma_thf_logic_formula : ','thf_logic_formula;
//%----<thf_top_level_type> appears after ":", where a type is being specified 
//%----for a term or variable. <thf_unitary_type> includes <thf_unitary_formula>, 
//%----so the syntax is very loose, but trying to be more specific about 
//%----<thf_unitary_type>s (ala the semantic rule) leads to reduce/reduce 
//%----conflicts. 
thf_atom_typing : typeable_atom ':' thf_top_level_type  |  '('thf_atom_typing')';
thf_top_level_type : thf_unitary_type  |  thf_mapping_type  |  thf_apply_type;
//%----Removed along with adding <thf_binary_type> to <thf_binary_formula>, for 
//%----TH1 polymorphic types with binary after quantification. 
//%----      | (<thf_binary_type>) 
thf_unitary_type : thf_unitary_formula;
//<thf_unitary_type>     :== <thf_atomic_type> | <th1_quantified_type> 
//<thf_atomic_type>      :== <type_constant> | <defined_type> | <variable> | <thf_mapping_type> | (<thf_atomic_type>) 
//<th1_quantified_type>  :== <type_quantifier> [<thf_variable_list>] : <thf_unitary_type> 
thf_apply_type : thf_apply_formula;
thf_binary_type : thf_mapping_type  |  thf_xprod_type  |  thf_union_type;
//%----Mapping is right-associative: o > o > o means o > (o > o). 
thf_mapping_type : thf_unitary_type Arrow thf_unitary_type  |  thf_unitary_type Arrow thf_mapping_type;
//%----Xproduct is left-associative: o * o * o means (o * o) * o. Xproduct can be replaced by tuple 
//%----types. 
thf_xprod_type : thf_unitary_type Star thf_unitary_type  |  thf_xprod_type Star thf_unitary_type;
//%----Union is left-associative: o + o + o means (o + o) + o. 
thf_union_type : thf_unitary_type Plus thf_unitary_type  |  thf_union_type Plus thf_unitary_type;
//%----Tuple types, e.g., [a,b,c], are allowed (by the loose syntax) as tuples. 
//%----Subtypes are not approved by the whiny community 
thf_subtype : atomic_type subtype_sign atomic_type;
//%----These are also used for NHF logic definitions 
thf_definition : thf_atomic_formula identical thf_logic_formula;
thf_sequent : thf_tuple gentzen_arrow thf_tuple;
//%-------------------------------------------------------------------------------------------------- 
//%----TFF formulae. 
tff_formula : tff_logic_formula  |  tff_atom_typing  |  tff_subtype;
tff_logic_formula : tff_unitary_formula  |  tff_unary_formula  |  tff_binary_formula  |  tff_defined_infix  |  txf_definition  |  txf_sequent;
//%----<tff_defined_infix> up here to avoid confusion in a = b | p, for TXF. 
//%----For plain TFF it can be in <tff_defined_atomic> 
tff_binary_formula : tff_binary_pair  |  tff_binary_list;
tff_binary_pair : tff_unit_formula nonassoc_connective tff_unit_formula;
//%----There's no intended precedence among associative connectives 
tff_binary_list : tff_or_formula  |  tff_and_formula;
tff_or_formula : tff_unit_formula Vline tff_unit_formula  |  tff_or_formula Vline tff_unit_formula;
tff_and_formula : tff_unit_formula '&' tff_unit_formula  |  tff_and_formula '&' tff_unit_formula;
tff_unit_formula : tff_unitary_formula  |  tff_unary_formula  |  tff_defined_infix;
tff_preunit_formula : tff_unitary_formula  |  tff_prefix_unary;
tff_unitary_formula : tff_quantified_formula  |  tff_atomic_formula  |  txf_unitary_formula  |  '('tff_logic_formula')';
txf_unitary_formula : variable;
tff_quantified_formula : tff_quantifier '['tff_variable_list']' ':' tff_unit_formula;
//%----Quantified formulae bind tightly, so cannot include infix formulae 
tff_variable_list : tff_variable  |  tff_variable','tff_variable_list;
tff_variable : tff_typed_variable  |  variable;
tff_typed_variable : variable ':' tff_atomic_type;
tff_unary_formula : tff_prefix_unary  |  tff_infix_unary;
//%FOR PLAIN TFF <fof_infix_unary> 
tff_prefix_unary : tff_unary_connective tff_preunit_formula;
tff_infix_unary : tff_unitary_term infix_inequality tff_unitary_term;
//%FOR PLAIN TFF <tff_atomic_formula>   ::= <fof_atomic_formula> 
tff_atomic_formula : tff_plain_atomic  |  tff_defined_atomic  |  tff_system_atomic;
tff_plain_atomic : constant  |  functor'('tff_arguments')';
//<tff_plain_atomic>     :== <proposition> | <predicate>(<tff_arguments>) 
tff_defined_atomic : tff_defined_plain;
//%---To avoid confusion in TXF a = b | p   | <tff_defined_infix> 
tff_defined_plain : defined_constant  |  defined_functor'('tff_arguments')'  |  nxf_atom  |  txf_let;
//% <txf_conditional> 
//%----<txf_conditional> is omitted from <tff_defined_plain> because $ite is 
//%----read simply as a <tff_atomic_formula> 
//<tff_defined_plain>    :== <defined_proposition> | <defined_predicate>(<tff_arguments>) | <nxf_atom> | <txf_conditional> | <txf_let> 
//%----This is the only one that is strictly a formula, type $o. In TXF, if the 
//%----LHS/RHS is a formula that does not look like a term, then it must be ()ed 
//%----per <tff_unitary_term>. If you put an un()ed formula that looks like a term 
//%----it will be interpreted as a term. 
tff_defined_infix : tff_unitary_term defined_infix_pred tff_unitary_term;
tff_system_atomic : system_constant  |  system_functor'('tff_arguments')';
//<tff_system_atomic>    :== <system_proposition> | <system_predicate>(<tff_arguments>) 
//%----<txf_conditional> is written and read as a <tff_defined_atomic> 
//<txf_conditional>      :== $ite(<tff_logic_formula>,<tff_term>,<tff_term>) 
txf_let : '$let('txf_let_types','txf_let_defns','tff_term')';
txf_let_types : tff_atom_typing  |  '['tff_atom_typing_list']';
tff_atom_typing_list : tff_atom_typing  |  tff_atom_typing','tff_atom_typing_list;
txf_let_defns : txf_let_defn  |  '['txf_let_defn_list']';
txf_let_defn : txf_let_LHS assignment tff_term;
txf_let_LHS : tff_plain_atomic  |  txf_tuple;
txf_let_defn_list : txf_let_defn  |  txf_let_defn','txf_let_defn_list;
nxf_atom : nxf_long_connective '@' '('tff_arguments')';
tff_term : tff_logic_formula  |  defined_term  |  txf_tuple;
tff_unitary_term : tff_atomic_formula  |  defined_term  |  txf_tuple  |  variable  |  '('tff_logic_formula')';
txf_tuple : '[]'  |  '['tff_arguments']';
tff_arguments : tff_term comma_tff_term*;
comma_tff_term : ','tff_term;
//%----<tff_atom_typing> can appear only at top level. 
tff_atom_typing : typeable_atom ':' tff_top_level_type  |  '('tff_atom_typing')';
tff_top_level_type : tff_atomic_type  |  tff_non_atomic_type;
tff_non_atomic_type : tff_mapping_type  |  tf1_quantified_type  |  '('tff_non_atomic_type')';
tf1_quantified_type : type_quantifier '['tff_variable_list']' ':' tff_monotype;
tff_monotype : tff_atomic_type  |  '('tff_mapping_type')'  |  tf1_quantified_type;
tff_unitary_type : tff_atomic_type  |  '('tff_xprod_type')';
tff_atomic_type : type_constant  |  defined_type  |  variable  |  type_functor'('tff_type_arguments')'  |  '('tff_atomic_type')'  |  txf_tuple_type;
tff_type_arguments : tff_atomic_type  |  tff_atomic_type','tff_type_arguments;
tff_mapping_type : tff_unitary_type Arrow tff_atomic_type;
tff_xprod_type : tff_unitary_type Star tff_atomic_type  |  tff_xprod_type Star tff_atomic_type;
//%----For TXF only 
txf_tuple_type : '['tff_type_list']';
tff_type_list : tff_top_level_type  |  tff_top_level_type','tff_type_list;
//%----Subtypes are not approved by the whiny community 
tff_subtype : atomic_type subtype_sign atomic_type;
//%----These are also used for NXF logic definitions 
txf_definition : tff_atomic_formula identical tff_term;
txf_sequent : txf_tuple gentzen_arrow txf_tuple;
//%-------------------------------------------------------------------------------------------------- 
//%----Typed non-classical here 
//%----Have to duplicate NHF and NXF because they lead to <thf_definition> and <txf_definition> 
nhf_long_connective : '{'ntf_connective_name'}'  |  '{'ntf_connective_name'('nhf_parameter_list')}';
nhf_parameter_list : nhf_parameter  |  nhf_parameter','nhf_parameter_list;
nhf_parameter : ntf_index  |  nhf_key_pair;
nhf_key_pair : thf_definition;
nxf_long_connective : '{'ntf_connective_name'}'  |  '{'ntf_connective_name'('nxf_parameter_list')}';
nxf_parameter_list : nxf_parameter  |  nxf_parameter','nxf_parameter_list;
nxf_parameter : ntf_index  |  nxf_key_pair;
nxf_key_pair : txf_definition;
ntf_connective_name : ntf_defined_connective  |  atomic_system_word;
ntf_defined_connective : atomic_defined_word;
//<ntf_connective_name>  :== $box | $dia | {$necessary} | {$possible} | {$obligatory} | {$permissible} | {$knows} | {$canKnow} | {$believes} | {$canBelieve} 
ntf_index : Hash tff_unitary_term;
ntf_short_connective : '[.]'  |  Less_sign'.'Arrow  |  '{.}'  |  '(.)';
//%----Short connectives are unary operators, cannot be indexed 
//%----                      | [<ntf_index>] | <Less_sign> <ntf_index> <Arrow> | 
//%----                      {<ntf_index>} 
//%----NXF logic specifications. Captured by <txf_definition> 
//%----NHF logic specifications are captured by <thf_definition> 
//<ntf_semantics_spec>   :== <ntf_logic_name> <identical> [<ntf_logic_spec_list>] 
//<ntf_logic_name>       :== $modal | $alethic_modal | $deontic_modal | $epistemic_modal | $doxastic_modal | $intuitionistic 
//<ntf_logic_spec_list>  :== <ntf_logic_spec> | <ntf_logic_spec>,<ntf_logic_spec_list> 
//<ntf_logic_spec>       :== <ntf_domains_spec> | <ntf_designation_spec> | <ntf_terms_spec> | <ntf_modalities_spec> | <ntf_time_spec> 
//<ntf_domains_spec>     :== $domains <identical> <ntf_domains_value> 
//<ntf_domains_value>    :== <ntf_domain_type> | [<ntf_domain_type_list>] 
//<ntf_domain_type>      :== $constant | $varying | $cumulative | $decreasing | <tff_atomic_type> <identical> <ntf_domains_value> 
//<ntf_domain_type_list> :== <ntf_domain_type> | <ntf_domain_type>,<ntf_domain_type_list> 
//<ntf_designation_spec> :== $designation <identical> <ntf_designation_value> 
//<ntf_designation_value>    :== <ntf_designation_type> | [<ntf_designation_type_list>] 
//<ntf_designation_type> :== $rigid | $flexible | <tff_atomic_type> <identical> <ntf_designation_value> 
//<ntf_designation_type_list> :== <ntf_designation_type> | <ntf_designation_type>,<ntf_designation_type_list> 
//<ntf_terms_spec>       :== $terms <identical> <ntf_terms_value> 
//<ntf_terms_value>      :== <ntf_terms_type> | [<ntf_terms_type_list>] 
//<ntf_terms_type>       :== $local | $global | <tff_atomic_type> <identical> <ntf_terms_value> 
//<ntf_terms_type_list>  :== <ntf_terms_type> | <ntf_terms_type>,<ntf_terms_type_list> 
//<ntf_modalities_spec>  :== $modalities <identical> <ntf_modalities_value> 
//<ntf_modalities_value> :== <ntf_modalities_type> | [<ntf_modalities_type_list>] 
//<ntf_modalities_type>  :== <ntf_modal_system> | <ntf_modal_axiom> | <tff_atomic_type> <identical> <ntf_modalities_value> 
//<ntf_modalities_type_list> :== <ntf_modalities_type> | <ntf_modalities_type>,<ntf_modalities_type_list> 
//<ntf_time_spec>        :== $time <identical> <ntf_time_value> 
//<ntf_time_value>       :== <ntf_time_type> | [<ntf_time_type_list>] 
//<ntf_time_type>        :== $reflexivity | $irreflexivity | $transitivity | $asymmetry | $anti_symmetry | $linearity | $forward_linearity | $backward_linearity | $beginning | $end | $no_beginning | $no_end | $density | $forward_discreteness | $backward_discreteness | <tff_atomic_type> <identical> <ntf_time_value> 
//<ntf_time_type_list>   :== <ntf_time_type> | <ntf_time_type>,<ntf_time_type_list> 
//<ntf_modal_system>     :== $modal_system_K | $modal_system_KB | $modal_system_K4 | $modal_system_K5 | $modal_system_K45 | $modal_system_KB5 | $modal_system_D | $modal_system_DB | $modal_system_D4 | $modal_system_D5 | $modal_system_D45 | $modal_system_M |  $modal_system_B | $modal_system_S4 | $modal_system_S5 | $modal_system_S5U 
//<ntf_modal_axiom>      :== $modal_axiom_K | $modal_axiom_M | $modal_axiom_B | $modal_axiom_D | $modal_axiom_4 | $modal_axiom_5 | $modal_axiom_CD | $modal_axiom_BoxM | $modal_axiom_C4 | $modal_axiom_C 
//%-------------------------------------------------------------------------------------------------- 
//%----TCF formulae. 
tcf_formula : tcf_logic_formula  |  tff_atom_typing;
tcf_logic_formula : tcf_quantified_formula  |  cnf_formula;
tcf_quantified_formula : '!' '['tff_variable_list']' ':' tcf_logic_formula;
//%-------------------------------------------------------------------------------------------------- 
//%----FOF formulae. 
fof_formula : fof_logic_formula  |  fof_sequent;
fof_logic_formula : fof_binary_formula  |  fof_unary_formula  |  fof_unitary_formula;
//%----Future answer variable ideas | <answer_formula> 
fof_binary_formula : fof_binary_pair  |  fof_binary_list;
fof_binary_pair : fof_unit_formula nonassoc_connective fof_unit_formula;
//%----There's no intended precedence among associative connectives 
fof_binary_list : fof_or_formula  |  fof_and_formula;
fof_or_formula : fof_unit_formula Vline fof_unit_formula  |  fof_or_formula Vline fof_unit_formula;
fof_and_formula : fof_unit_formula '&' fof_unit_formula  |  fof_and_formula '&' fof_unit_formula;
fof_unary_formula : unary_connective fof_unit_formula  |  fof_infix_unary;
//%----<fof_term> != <fof_term> is equivalent to ~ <fof_term> = <fof_term> 
fof_infix_unary : fof_term infix_inequality fof_term;
//%----<fof_unitary_formula> are in ()s or do not have a connective 
fof_unit_formula : fof_unitary_formula  |  fof_unary_formula;
fof_unitary_formula : fof_quantified_formula  |  fof_atomic_formula  |  '('fof_logic_formula')';
//%----All variables must be quantified 
fof_quantified_formula : fof_quantifier '['fof_variable_list']' ':' fof_unit_formula;
fof_variable_list : variable  |  variable','fof_variable_list;
fof_atomic_formula : fof_plain_atomic_formula  |  fof_defined_atomic_formula  |  fof_system_atomic_formula;
fof_plain_atomic_formula : fof_plain_term;
//<fof_plain_atomic_formula> :== <proposition> | <predicate>(<fof_arguments>) 
fof_defined_atomic_formula : fof_defined_plain_formula  |  fof_defined_infix_formula;
fof_defined_plain_formula : fof_defined_plain_term;
//<fof_defined_plain_formula> :== <defined_proposition> | <defined_predicate>(<fof_arguments>) 
fof_defined_infix_formula : fof_term defined_infix_pred fof_term;
//%----System terms have system specific interpretations 
fof_system_atomic_formula : fof_system_term;
//%----<fof_system_atomic_formula>s are used for evaluable predicates that are 
//%----available in particular tools. The predicate names are not controlled by 
//%----the TPTP syntax, so use with due care. Same for <fof_system_term>s. 
//%----FOF terms. 
fof_plain_term : constant  |  functor'('fof_arguments')';
//%----Defined terms have TPTP specific interpretations 
fof_defined_term : defined_term  |  fof_defined_atomic_term;
fof_defined_atomic_term : fof_defined_plain_term;
//%----None yet             | <defined_infix_term> 
//%----None yet <defined_infix_term> ::= <fof_term> <defined_infix_func> <fof_term> 
//%----None yet <defined_infix_func> ::= 
fof_defined_plain_term : defined_constant  |  defined_functor'('fof_arguments')';
//%----System terms have system specific interpretations 
fof_system_term : system_constant  |  system_functor'('fof_arguments')';
//%----Arguments recurse back to terms (this is the FOF world here) 
fof_arguments : fof_term  |  fof_term','fof_arguments;
//%----These are terms used as arguments. Not the entry point for terms because 
//%----<fof_plain_term> is also used as <fof_plain_atomic_formula>. The <tff_ 
//%----options are for only TFF, but are here because <fof_plain_atomic_formula> 
//%----is used in <fof_atomic_formula>, which is also used as 
//%----<tff_atomic_formula>. 
fof_term : fof_function_term  |  variable;
fof_function_term : fof_plain_term  |  fof_defined_term  |  fof_system_term;
//%-------------------------------------------------------------------------------------------------- 
//%----This section is the FOFX syntax. Not yet in use. 
fof_sequent : fof_formula_tuple gentzen_arrow fof_formula_tuple  |  '('fof_sequent')';
fof_formula_tuple : '[]'  |  '['fof_formula_tuple_list']';
fof_formula_tuple_list : fof_logic_formula comma_fof_logic_formula*;
comma_fof_logic_formula : ','fof_logic_formula;
//%-------------------------------------------------------------------------------------------------- 
//%----CNF formulae (variables implicitly universally quantified) 
cnf_formula : cnf_disjunction  |  '(' cnf_formula ')';
cnf_disjunction : cnf_literal  |  cnf_disjunction Vline cnf_literal;
cnf_literal : fof_atomic_formula  |  '~' fof_atomic_formula  |  '~' '('fof_atomic_formula')'  |  fof_infix_unary;
//%-------------------------------------------------------------------------------------------------- 
//%----Connectives - THF 
thf_quantifier : tff_quantifier  |  th0_quantifier  |  type_quantifier;
thf_unary_connective : unary_connective  |  ntf_short_connective;
//%----TH0 quantifiers are also available in TH1 
th0_quantifier : '^'  |  '@+'  |  '@-';
//%----Connectives - THF and TFF 
type_quantifier : '!>'  |  '?'*;
subtype_sign : '<<';
//%----Connectives - TFF 
tff_unary_connective : unary_connective  |  ntf_short_connective;
tff_quantifier : fof_quantifier  |  Hash;
//%----Connectives - FOF 
fof_quantifier : '!'  |  '?';
nonassoc_connective : '<=>'  |  '=>'  |  '<='  |  '<~>'  |  '~'Vline  |  '~&';
assoc_connective : Vline  |  '&';
unary_connective : '~';
//%----The seqent Arrow 
gentzen_arrow : '-->';
assignment : ':=';
identical : '==';
//%----Types for all language types 
typeable_atom : constant  |  Distinct_object;
atomic_type : typeable_atom  |  defined_constant  |  system_type;
//%----I wish I could use ... 
//<atomic_type>          :== <type_constant> | <defined_type> | <system_type> 
//%----... but that gives reduce conflicts because ... 
//%----<type_constant> expands to <atomic_word>, but happily so does <constant> 
//%----<defined_type>  expands to <atomic_defined_word>, but happily so does <defined_constant> 
//%----Allowing <Distinct_object> as an <atomic_type> is plain wrong. 
//%----Types for THF and TFF 
type_constant : type_functor;
type_functor : atomic_word;
defined_type : atomic_defined_word;
//<defined_type>         :== $oType | $o | $iType | $i | $tType | $Real | $rat | $int 
//%----$oType/$o is the Boolean type, i.e., the type of $true and $false. 
//%----$iType/$i is non-empty type of individuals, which may be finite or 
//%----infinite. $tType is the type of all types. $Real is the type of <Real>s. 
//%----$rat is the type of <Rational>s. $int is the type of <Signed_integer>s 
//%----and <Unsigned_integer>s. 
system_type : atomic_system_word;
//<proposition>          :== <predicate> 
//<predicate>            :== <atomic_word> 
//<defined_proposition>  :== <defined_predicate> 
//<defined_proposition>  :== $true | $false 
//<defined_predicate>    :== <atomic_defined_word> 
//<defined_predicate>    :== $distinct | $less | $lesseq | $greater | $greatereq | $is_int | $is_rat 
//%----$distinct is part of the TXF and THF languages. $distinct is always written in functional  
//%----form, even in THF. It takes one or more terms of the same type as arguments, and indicates  
//%----that the arguments are pairwise !=. 
defined_infix_pred : infix_equality;
//<system_proposition>   :== <system_predicate> 
//<system_predicate>     :== <atomic_system_word> 
infix_equality : '=';
infix_inequality : '!=';
constant : functor;
functor : atomic_word;
defined_constant : defined_functor;
defined_functor : atomic_defined_word;
//<defined_functor>      :== $uminus | $sum | $difference | $product | $quotient | $quotient_e | $quotient_t | $quotient_f | $remainder_e | $remainder_t | $remainder_f | $floor | $ceiling | $truncate | $round | $abs | $to_int | $to_rat | $to_real 
system_constant : system_functor;
system_functor : atomic_system_word;
th1_defined_term : '!!'  |  '??'  |  '@@+'  |  '@@-'  |  '@=';
defined_term : number  |  Distinct_object;
variable : Upper_word;
//%-------------------------------------------------------------------------------------------------- 
//%----Formula sources 
//%----Expanded semantic rules for IDV. It was <source>               ::= <general_term> 
source : dag_source  |  internal_source  |  external_source  |  '['sources']';
//%----A possible <source>, but conflicts with <dag_source>, is unknown 
//%----Alternative sources are recorded like this, thus allowing representation 
//%----of alternative derivations with shared parts. 
sources : source  |  source','sources;
//%----Only a <dag_source> can be a <name>, i.e., derived formulae can be 
//%----identified by a <name> or an <inference_record> 
dag_source : name  |  inference_record;
inference_record : 'inference('inference_rule','useful_info','parents')';
inference_rule : atomic_word;
//%----Examples are          deduction | modus_tollens | modus_ponens | rewrite | resolution | 
//%----                      paramodulation | factorization | cnf_conversion | cnf_refutation | ... 
internal_source : 'introduced('intro_type','useful_info','parents')';
intro_type : atomic_word;
//<intro_type>           :== definition | tautology | assumption | theory 
external_source : file_source;
file_source : 'file('file_name file_info')';
file_info : ','name  |  nothing;
//%----<parents> can be empty in cases when there is a justification for a tautologous theorem. In 
//%----cases when a tautology is introduced as a leaf, e.g., for splitting, then use an 
//%----<internal_source>. 
parents : '[]'  |  '['parent_list']';
parent_list : parent_info comma_parent_info*;
comma_parent_info : ','parent_info;
parent_info : source parent_details;
parent_details : ':'general_term  |  nothing;
//%----Useful info fields 
optional_info : ','useful_info  |  nothing;
useful_info : general_list;
//<useful_info>          :== [] | [<info_items>] 
//<info_items>           :== <info_item> <comma_info_item>* 
//<comma_info_item>      :== ,<info_item> 
//<info_item>            :== <formula_item> | <inference_item> | <general_function> 
//%----Useful info for formula records 
//<formula_item>         :== <description_item> | <iquote_item> 
//<description_item>     :== description(<atomic_word>) 
//<iquote_item>          :== iquote(<atomic_word>) 
//%----<iquote_item>s are used for recording exactly what the system output about 
//%----the inference step. In the future it is planned to encode this information 
//%----in standardized forms as <parent_details> in each <inference_record>. 
//%----Useful info for inference records 
//<inference_item>       :== <inference_status> | <assumptions_record> | <new_symbol_record> | <refutation> 
//<inference_status>     :== status(<status_value>) | <inference_info> 
//%----These are the success status values from the SZS ontology. The most 
//%----commonly used values are: 
//%----  thm - Every model of the parent formulae is a model of the inferred formula. Regular logical 
//%----        consequences. 
//%----  cth - Every model of the parent formulae is a model of the negation of the inferred formula. 
//%----        Used for negation of conjectures in FOF to CNF conversion. 
//%----  esa - There exists a model of the parent formulae iff there exists a model of the inferred 
//%----        formula. Used for Skolemization steps. 
//%----For the full hierarchy see https://szs.tptp.org 
//%----<inference_info> is used to record standard information associated with an arbitrary inference 
//%----rule. The <inference_rule> is the same as the <inference_rule> of the <inference_record>. The 
//%----<atomic_word> indicates the information being recorded in the <general_list>. The 
//%----<atomic_word> are (loosely) set by TPTP conventions, and include esplit, sr_split, and 
//%----discharge. 
//<inference_info>       :== <inference_rule>(<atomic_word>,<general_list>) 
//%----An <assumptions_record> lists the names of assumptions upon which this 
//%----inferred formula depends. These must be discharged in a completed proof. 
//<assumptions_record>   :== assumptions([<name_list>]) 
//%----A <refutation> record names a file in which the inference recorded here 
//%----is recorded as a proof by refutation. 
//<refutation>           :== refutation(<file_source>) 
//%----A <new_symbol_record> provides information about a newly introduced symbol. 
//<new_symbol_record>    :== new_symbols(<atomic_word>,[<new_symbol_list>]) 
//<new_symbol_list>      :== <principal_symbol> | <principal_symbol>,<new_symbol_list> 
//%----Principal symbols are predicates, functions, variables 
//<principal_symbol>     :== <functor> | <variable> 
//%----Include directives 
include : 'include('file_name include_optionals').';
include_optionals : nothing  |  ','formula_selection  |  ','formula_selection','space_name;
formula_selection : '['name_list']'  |  Star;
name_list : name  |  name','name_list;
space_name : name;
//%----Non-logical data 
general_term : general_data  |  general_data':'general_term  |  general_list;
general_data : atomic_word  |  general_function  |  variable  |  number  |  Distinct_object  |  formula_data;
general_function : atomic_word'('general_terms')';
//%----A <general_data> bind() term is used to record a variable binding in an 
//%----inference, as an element of the <parent_details> list. 
//<general_data>         :== bind(<variable>,<formula_data>) | bind_type(<variable>,<bound_type>) 
//<bound_type>           :== $thf(<thf_top_level_type>) | $tff(<tff_top_level_type>) 
formula_data : '$thf('thf_formula')'  |  '$tff('tff_formula')'  |  '$fof('fof_formula')'  |  '$cnf('cnf_formula')'  |  '$fot('fof_term')';
general_list : '[]'  |  '['general_terms']';
general_terms : general_term comma_general_term*;
comma_general_term : ','general_term;
//%----General purpose 
//%----Integer names are expected to be unsigned, but lex stuff prevents this .. 
//%----<name>                 ::= <atomic_word> | <Unsigned_integer> 
name : atomic_word  |  Integer;
atomic_word : Lower_word  |  Single_quoted  |  Back_quoted;
//%----<Single_quoted>s are the enclosed <atomic_word> without the quotes. Therefore the <Lower_word> 
//%----<atomic_word> cat and the <Single_quoted> <atomic_word> 'cat' are the same, but <numbers>s and 
//%----<variable>s are not <Lower_word>s, so '123' is different from 123, and 'X' is different from  
//%----X. Quotes can be removed from a <Single_quoted> <atomic_word>, and is recommended if doing so  
//%----produces a Lower_word> <atomic_word>. 
atomic_defined_word : Dollar_word;
atomic_system_word : Dollar_dollar_word;
number : Integer  |  Rational  |  Real;
//%----Numbers are always interpreted as themselves, and are thus implicitly 
//%----distinct if they have different values, e.g., 1 != 2 is an implicit axiom. 
//%----All numbers are base 10 at the moment. 
file_name : atomic_word;
nothing : ;
//%-------------------------------------------------------------------------------------------------- 
//%----Rules from here on down are for defining tokens (terminal symbols) of the grammar, assuming 
//%----they will be recognized by a lexical scanner. 
//%----A ::- rule defines a token, a ::: rule defines a macro that is not a token. Usual regexp 
//%----notation is used. Single characters are always placed in []s to disable any special meanings 
//%----(for uniformity this is done to all characters, not only those with special meanings). 
//%----These are tokens that appear in the syntax rules above. No rules defined here because they 
//%----appear explicitly in the syntax rules, except that <Vline>, <Star>, <Plus>, <Hash> denote 
//%----"|", "*", "+", "#", respectively. 
//%----Keywords:    thf tff fof cnf include 
//%----Punctuation: ( ) , . [ ] : 
//%----Operators:   ! ? ~ & | <=> => <= <~> ~| ~& * + 
//%----Predicates:  = != $true $false $arithmetic_stuff 
//%----<Single_quoted>s contain visible characters. \ is the escape character for ' and \, i.e., 
//%----\' is not the end of the <Single_quoted>. The token does not include the outer quotes, e.g., 
//%----'cat' and cat are the same. See <atomic_word> for information about stripping the quotes. 
//%---Space and visible characters upto ~, except " and \ <Distinct_object>s contain visible 
//%----characters. \ is the escape character for " and \, i.e., \" is not the end of the 
//%----<Distinct_object>. <Distinct_object>s are different from (but may be equal to) other tokens, 
//%----e.g., "cat" is different from 'cat' and cat. Distinct objects are always interpreted as 
//%----themselves, so if they are different they are unequal, e.g., "Apple" != "Microsoft" is 
//%----implicit. 
//%----Numbers. Note only the <Real>, <Rational>, and <Integer> are accessible. 
//%----Tokens used in syntax, and cannot be character classes 
//%----For lex/yacc there cannot be spaces on either side of the | here 
//%----Defined comments are a convention used for annotations that are used as additional input for 
//%----systems. They look like comments, but start with %$ or /*$. A wily user of the syntax can 
//%----notice the $ and extract information from the "comment" and pass that on as input to the 
//%----system. They are analogous to pragmas in programming languages. To extract these separately 
//%----from regular comments, the rules are: 
//%----  <defined_comment>    ::- <def_comment_line>|<def_comment_block> 
//%----  <def_comment_line>   ::: [%]<Dollar> <Comment_char>* 
//%----  <def_comment_block>  ::: [/][*]<Dollar> <Not_star_slash>[*][*]*[/] 
//%----A string that matches both <defined_comment> and <comment> should be recognized as 
//%----<defined_comment>, so put these before <comment>. Defined comments that are in use include: 
//%----    TO BE ANNOUNCED 
//%----System comments are a convention used for annotations that may be used as additional input to 
//%----a specific system. They look like comments, but start with %$$ or /*$$. A wily user of the 
//%----syntax can notice the $$ and extract information from the "comment" and pass that on as input 
//%----to the system. The specific system for which the information is intended should be identified 
//%----after the $$, e.g., /*$$Otter 3.3: Demodulator */ To extract these separately from regular 
//%----comments, the rules are: 
//%----  <system_comment>     ::- <sys_comment_line>|<sys_comment_block> 
//%----  <sys_comment_line>   ::: [%]<Dollar> <Dollar> <Comment_char>* 
//%----  <sys_comment_block>  ::: [/][*]<Dollar> <Dollar> <Not_star_slash>[*][*]*[/] 
//%----A string that matches both <system_comment> and <defined_comment> should 
//%----be recognized as <system_comment>, so put these before <defined_comment>. 
//%----Character classes 
//%---Space and visible characters upto ~, except ' and \ 
//%----<printable_char> is any printable ASCII character, codes 32 (space) to 126 (tilde). 
//%----<printable_char> does not include tabs, newlines, bells, etc. The use of . allows 
//%----tab, so this is a bit loose. 
//%-------------------------------------------------------------------------------------------------- 
