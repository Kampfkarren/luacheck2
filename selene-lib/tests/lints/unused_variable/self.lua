local Foo = {}

function Foo:A() end
function Foo.B() end
function Foo    :    C() end

return Foo
