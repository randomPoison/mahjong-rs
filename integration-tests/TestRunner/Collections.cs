using System.Collections.Generic;
using Xunit;

namespace TestRunner
{
    public class Collections
    {
        [Fact]
        public void ReturnVecInt()
        {
            var expected = IntegrationTests.ReturnVecI32();
            var actual = new List<int>() { 1, 2, 3, 4 };
            Assert.Equal(expected, actual);
        }

        [Fact]
        public void ReturnVecIntRepeated()
        {
            for (var count = 0; count < 100_000; count += 1)
            {
                var expected = IntegrationTests.ReturnVecI32();
                var actual = new List<int>() { 1, 2, 3, 4 };
                Assert.Equal(expected, actual);
            }
        }

        [Fact]
        public void ReturnVecFloat()
        {
            var expected = IntegrationTests.ReturnVecF32();
            var actual = new List<float>() { 1.0f, 2.1f, 3.123f, 4.00000004f };
            Assert.Equal(expected, actual);
        }

        [Fact]
        public void ReturnVecBool()
        {
            var expected = IntegrationTests.ReturnVecBool();
            var actual = new List<bool>() { true, false, true, true };
            Assert.Equal(expected, actual);
        }

        // TODO: Re-enable test case once we fix support for passing list of handle types.
        // [Fact]
        public void ReturnHandleList()
        {
            var items = IntegrationTests.ReturnHandleVec();
            Assert.Equal(2, items.Count);
            Assert.Equal(33, items[0].Bar());
            Assert.Equal(12345, items[1].Bar());
        }


        [Fact]
        public void ReturnStructList()
        {
            var items = IntegrationTests.ReturnStructVec();
            Assert.Equal(2, items.Count);
            Assert.Equal(33, items[0].Bar);
            Assert.Equal(12345, items[1].Bar);
        }
    }
}
