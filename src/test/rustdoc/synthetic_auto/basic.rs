// @has basic/struct.Foo.html
// @has - '//h3[@class="code-header in-band"]' 'impl<T> Send for Foo<T> where T: Send'
// @has - '//h3[@class="code-header in-band"]' 'impl<T> Sync for Foo<T> where T: Sync'
// @has - '//h3[@class="code-header in-band"]' 'impl<T> FinalizerSafe for Foo<T> where T: FinalizerSafe'
// @count - '//*[@id="implementations-list"]//*[@class="impl has-srclink"]' 0
// @count - '//*[@id="synthetic-implementations-list"]//*[@class="impl has-srclink"]' 8
pub struct Foo<T> {
    field: T,
}
