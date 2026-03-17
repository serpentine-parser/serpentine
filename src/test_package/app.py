""""""

import math  # import_statement, define_name (math)

GLOBAL_CONSTANT = 10  # define_name


class Engine:  # define_name, enter_scope (class)
    """Represents an engine that can be started."""

    def start(self):  # define_name, enter_scope (function)
        print("Engine started")  # use_name (print), call_expression

    # exit_scope (function)


# exit_scope (class)


class Car:  # define_name, enter_scope (class)
    def __init__(self, engine):  # define_name, enter_scope (function)
        self.engine = engine  # attribute_access, define_name (engine param), use_name

    # exit_scope (function)

    def drive(self, speed):  # define_name, enter_scope (function)
        if speed > GLOBAL_CONSTANT:  # control_block, use_name (speed, GLOBAL_CONSTANT)
            self.engine.start()  # attribute_access, call_expression
        else:
            self.stop()  # attribute_access, call_expression

    # exit_scope (function)

    def stop(self):  # define_name, enter_scope (function)
        print("Car stopped")  # use_name, call_expression

    # exit_scope(function)


def build_car():  # define_name, enter_scope (function)
    eng = Engine()  # define_name, call_expression, use_name (Engine)
    car = Car(eng)  # define_name, call_expression, use_name (Car, eng)
    return car  # use_name


def main():  # define_name, enter_scope (function)
    car = build_car()  # define_name, call_expression, use_name
    car.drive(math.sqrt(25))  # attribute_access, call_expression, use_name


if __name__ == "__main__":  # control_block, use_name
    main()  # call_expression, use_name
