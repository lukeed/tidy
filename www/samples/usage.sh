# print one file, formatted
$ tidy index.html

# list what isn't formatted; exits 1 if any
$ tidy check src

# rewrite everything in place
$ tidy up .

# only what this branch changed
$ tidy up --changed
