package proc

type errors struct {
	errs []error
}

func (e *errors) append(err error) {
	e.errs = append(e.errs, err)
}

func (e *errors) len() int {
	return len(e.errs)
}

func (e *errors) Error() string {
	str := ""
	for _, err := range e.errs {
		str += err.Error()
	}
	return str
}
