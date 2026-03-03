pub struct Kalman {
    gain: f32,
    process_variance: f32,
    estimation_error: f32,
    measurement_error: f32,
    current_estimation: f32,
    last_estimation: f32,
}

impl Kalman {
    /// Creates new instance of the Kalman filter
    ///
    /// measurement_error: How much do we expect to our measurement vary
    /// process_variance: How fast your measurement moves. Usually 0.001 - 1
    /// initial_value: Where the filter starts calculation
    pub fn new(measurement_error: f32, process_variance: f32, initial_value: f32) -> Self {
        // Can be initialized with the same value as measurement_error,
        // since the kalman filter will adjust its value.
        let estimation_error = measurement_error;
        let gain = estimation_error / (estimation_error + measurement_error);

        Self {
            gain,
            process_variance,
            estimation_error,
            measurement_error,
            current_estimation: initial_value,
            last_estimation: initial_value,
        }
    }

    pub fn update(&mut self, value: f32) {
        self.gain = self.estimation_error / (self.estimation_error + self.measurement_error);

        let value_change = self.gain * (value - self.last_estimation);
        self.current_estimation = self.last_estimation + value_change;

        let estimation_change =
            f32::abs(self.last_estimation - self.current_estimation) * self.process_variance;
        self.estimation_error = (1.0 - self.gain) * self.estimation_error + estimation_change;

        self.last_estimation = self.current_estimation;
    }

    pub fn value(&self) -> f32 {
        self.current_estimation
    }
}
