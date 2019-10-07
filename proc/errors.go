package proc

// MultiErrors is a wrap for multiple other errors.
type MultiErrors struct {
	es []error
}

// Error concats all inner errors into a single string.
func (e *MultiErrors) Error() string {
	if e == nil {
		return "<nil>"
	}

	var str string
	for _, err := range e.es {
		str += err.Error()
	}
	return str
}
