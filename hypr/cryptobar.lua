local window = {class = "^(dev\\.akayashuu\\.cryptobar)$"}

hl.window_rule({match = window, float = true})
hl.window_rule({match = window, size = {640, 940}})
hl.window_rule({match = window, center = true})
hl.window_rule({match = window, animation = "popin"})
