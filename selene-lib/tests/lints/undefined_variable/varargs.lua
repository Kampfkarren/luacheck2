local args = ...

return function()
    local shouldntExist = ...
    return function(...)
        local shouldExist = ...
        return function()
            local alsoShouldntExist = ...
        end
    end
end