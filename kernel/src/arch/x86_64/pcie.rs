#[macro_export]
macro_rules! pcie_port_readonly {
    ($name:ident, $output_type:ty, | $self:ident | $addr:block) => {
        paste::paste! {
            pub fn [<read_$name>](&$self) -> $output_type {
                let address: *mut $output_type = $addr;
                unsafe { address.read_volatile() }
            }
        }
    };

    ($name:ident, $output_type:ty, | $self:ident | $addr:block, || $head:block, || $tail:block) => {
        paste::paste! {
            pub fn [<read_$name>](&$self) -> $output_type {
                $head;

                let address: *mut $output_type = $addr;
                let res = unsafe { address.read_volatile() };

                $tail;

                res
            }
        }
    };
}

#[macro_export]
macro_rules! pcie_port_writeonly {
    ($name:ident, $input_type:ty, | $self:ident | $addr:block) => {
        paste::paste! {
            pub fn [<write_$name>](&mut $self, input: $input_type) {
                let address: *mut $input_type = $addr;
                unsafe { address.write_volatile(input) }
            }
        }
    };

    ($name:ident, $input_type:ty, | $self:ident | $addr:block, || $head:block, || $tail:block) => {
        paste::paste! {
            pub fn [<write_$name>](&mut $self, input: $input_type) {
                $head;

                let address: *mut $input_type = $addr;
                unsafe { address.write_volatile(input) }

                $tail;
            }
        }
    };
}

#[macro_export]
macro_rules! pcie_port_readwrite {
    ($($args:tt)*) => {
        $crate::pcie_port_readonly!($($args)*);
        $crate::pcie_port_writeonly!($($args)*);
    };
}
