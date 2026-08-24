def map_test():
    m = {}
    for i in range(10000):
        m[i] = i * 2
    sum = 0
    for i in range(10000):
        sum += m[i]
    return sum
print(map_test())
