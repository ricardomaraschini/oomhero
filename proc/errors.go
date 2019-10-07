package proc

import "strings"

// MultiErrors is a wrap for multiple other errors.
type MultiErrors struct {
	es []error
}

// Error concats all inner errors into a single string.
func (e *MultiErrors) Error() string {
	if e == nil {
		return "<nil>"
	}

	strs := make([]string, len(e.es))
	for i, err := range e.es {
		strs[i] = err.Error()
	}
	return strings.Join(strs, ",")
}
