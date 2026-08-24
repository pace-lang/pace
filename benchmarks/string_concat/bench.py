def concat_test():
    s = []
    for i in range(10000):
        s.append("a")
    return "".join(s)
print(len(concat_test()))
