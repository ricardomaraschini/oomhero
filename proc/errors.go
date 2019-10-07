package proc

// MultiError is a wrap for multiple other errors.
type MultiError struct {
	es []error
}

// Error concats all inner errors into a single string.
func (e *MultiError) Error() string {
	if e == nil {
		return "<nil>"
	}

	var str string
	for _, err := range e.es {
		str += err.Error()
	}
	return str
}
