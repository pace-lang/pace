class Person:
    def __init__(self, age, weight):
        self.age = age
        self.weight = weight
        
    def get_value(self):
        return self.age + self.weight

def class_test():
    sum_val = 0
    for i in range(1000000):
        p = Person(i, i + 1)
        sum_val += p.get_value()
    return sum_val

if __name__ == "__main__":
    print(class_test())
