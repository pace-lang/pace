def loop_sum():
    sum = 0
    i = 0
    while i < 10000000:
        sum += i
        i += 1
    return sum
print(loop_sum())
