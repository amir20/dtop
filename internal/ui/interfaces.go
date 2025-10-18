package ui

// StatusBar is an interface for pages that display a status bar.
// Pages implementing this interface will have their height reduced by 1
// to account for the status bar when window size is calculated.
type StatusBar interface {
	// StatusBar returns the rendered status bar string
	StatusBar() string
}
